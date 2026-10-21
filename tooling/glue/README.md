# Glue specs

`roc glue` is the compiler's official way to generate a platform host's ABI
layer: a glue *spec* (a Roc app against the `glue` platform) receives the
compiler-emitted type table and writes host-language source. The compiler repo
ships `src/glue/src/RustGlue.roc` for Rust hosts, and its README is explicit
that platform code should consume generated glue rather than hand-rolling ABI
bindings, because the layout, ownership and refcounting contracts are compiler
internals.

## The official Rust glue (works, not adopted yet)

`RustGlue.roc` runs against this platform and writes one Rust file:

```bash
# fetch it from the roc repo at the commit the pinned nightly was built from
curl -s -o /tmp/RustGlue.roc \
  https://raw.githubusercontent.com/roc-lang/roc/<nightly-sha>/src/glue/src/RustGlue.roc
roc glue /tmp/RustGlue.roc tooling/glue/generated platform/main.roc
# -> tooling/glue/generated/roc_platform_abi.rs
```

`tooling/glue/generated/roc_platform_abi.rs` is checked in as the reference the
hand-written ABI is measured against. It contains:

- `RocStr`, `RocListWith<T, ELEMENTS_REFCOUNTED>`, `RocDec` and the rest of the
  ABI-visible types, with the layout documented in the same terms this repo
  reverse-engineered by hand (capacity shifted left by one, seamless-slice tag
  in the capacity word, refcount one word before the bytes);
- retain/release plans (`RocStrRelease`, `RocListRelease`, `RocListSpineRelease`,
  `RocOwned`), including the recursive teardown rules;
- the **hosted symbols** the host must export (`rocFx*`) with the exact direct C
  ABI signatures and per-argument ownership notes;
- **owning wrappers for the provided entrypoints** (`main_*_owned`) plus
  `const _: () = assert!(...)` size/alignment checks, which is exactly the call
  side `host/src/lib.rs` hand-writes with `MaybeUninit` + `mem::forget`.

Adopting it would delete `host/src/roc_std.rs` and the hand-written ownership
handling, and make a future ABI change a regeneration instead of a rewrite. The
work, in order:

1. include the generated file in the host crate (it needs the `no_roc_std_helpers`
   cfg the platform build sets) and switch `host/src/lib.rs` from
   `roc_std::RocStr` to the generated type — mechanical, ~40 call sites;
2. use `main_*_owned` for the entrypoint calls and the generated `decref` for
   arguments, replacing the manual `mem::forget`/`into_result` dance;
3. keep the allocator exports (`roc_alloc`/`roc_dealloc`/`roc_realloc`,
   `cabi_realloc`) — the generated file declares them as externs with the
   contract spelled out;
4. re-run the harness and e2e, then delete `roc_std.rs` and the layout constants
   this repo maintains.

## The diagnostic specs

The two specs below dump what *this* platform declares, which is how the
`host/src/roc_std.rs` layout constants were derived and how a nightly bump was
verified before the official generator was available:

```bash
roc glue tooling/glue/abi_facts.roc   /tmp/glue-out platform/main.roc
cat /tmp/glue-out/abi_facts.txt
```

They run on `nightly-2026-09-19-d025939` and **not** on the pinned
`nightly-2026-10-19-601b54c`: the glue platform's API moved (`pf.FunctionRepr`
is gone; `pf.FunctionInfo`, `pf.FunctionSignature`, `pf.ProvidedExport` and
`pf.HostedFunctionInfo` replaced it) and the new compiler's type-module
convention rejects the `import pf.X exposing [X]` clauses these specs use. Port
them only if the diagnostic output is still wanted after the official generator
is adopted — the generator already emits the layout assertions and ownership
rules that made the table below worth keeping.

## ABI facts (verified against this platform)
`abi_facts.roc` output, pointer width 32:

| Fact | Value |
| --- | --- |
| `Str` | 3 words (12 bytes), `RcRefcounted`, field order `bytes, capacity, length` |
| `Try(Str, Str)` | 16 bytes, discriminant byte at offset 12, `Err = 0`, `Ok = 1` |
| `Try(U32, Str)` | 16 bytes, discriminant at 12, `Err` payload 12 bytes, `Ok` payload 4 bytes |
| `Try({}, Str)` | 16 bytes, discriminant at 12, `Ok` payload 0 bytes |
| `main_init_for_host!` | `Fn([Str] -> Try(Str, Str))` |
| `main_handle_message_for_host!` | `Fn([Str, Str] -> Try(Str, Str))` |
| `main_handle_tool_call_for_host!` | `Fn([Str, Str] -> Try(Str, Str))` |
| `main_metadata_for_host` | `Fn([Unit] -> Str)` |
| hosted effects | `rocFxLog(U8, Str) -> {}`, `rocFxGetWorkerId({}) -> Str`, `rocFxRpcInvoke(Str, Str, Str) -> Try(Str, Str)`, `rocFxSetPersistence(U8) -> {}`, `rocFxHttpRequest(Str) -> Try(Str, Str)`, `rocFxWsConnect(Str) -> Try(U32, Str)`, `rocFxWsSend(U32, Str) -> Try({}, Str)`, `rocFxWsReceive(U32) -> Try(Str, Str)`, `rocFxWsClose(U32) -> Try({}, Str)`, `rocFxSleepMillis(U64) -> {}`, `rocFxNowMillis({}) -> U64` |

Two of these corrected `host/src/roc_std.rs`: a heap `Str` carries its length in
**word 3** (the host read word 2, so every string longer than 11 bytes came back
garbled, which is why the agent metadata failed to parse and every component
reported the fallback `roc-agent` type), and `Try` uses **1 for `Ok`** (the host
had it inverted, so successful calls surfaced as errors).

## What the facts do not cover

`roc_alloc` / `roc_dealloc` / `roc_realloc` are declared by the compiler
(`env::roc_alloc(size, align) -> ptr`, `env::roc_dealloc(ptr, align)`,
`env::roc_realloc(ptr, new_size, align) -> ptr`) but their *memory protocol* —
how much space the host must leave before the data pointer, and what the runtime
stores there — is not part of the emitted facts. It lives in the compiler's
builtins instead (`src/builtins/str.zig`, `src/builtins/utils.zig`,
`src/host_alloc/`), and `host/src/roc_std.rs` now mirrors it exactly:

- `roc_alloc` returns a pointer the runtime treats as the *allocation*; it
  writes a refcount one word before the bytes it hands out, and `roc_dealloc`
  is called with that same pointer (for a string, the refcount word, not the
  bytes). Our own size header sits entirely before it.
- A `Str`'s words are `{ bytes, capacity << 1, length }`; bit 0 of the capacity
  word is `SEAMLESS_SLICE_TAG`, and a slice stores the *original* string's bytes
  pointer there, so releasing a slice must release that allocation.
- Refcount `0` means the data has whole-program lifetime (a constant) and must
  never be freed; a slice shares the refcount of the allocation it points into.

The component-model boundary uses `cabi_realloc`, which wit-bindgen's glue frees
through Rust's global allocator and which therefore must be a plain allocation
with no Roc header. Mixing the two is what produced the long-lived `initialize`
trap (see the Development status section of the root README).

## What is still missing

A full Rust generator (the port of the upstream `RustGlue.roc`) that turns those
facts into Rust structs and call wrappers. The copy of that spec in the older
`roc-practice/roc-golem` project was written against a July 2026 glue platform
and does **not** run on the pinned nightly; porting it needs:

1. `TypeRepr` gained variants (e.g. `RocI16x8`) — the spec's matches must be
   exhaustive.
2. Tag-union facts moved out of `TagUnionRepr` / `TypeTable` into `AbiLayout`
   (`tag_union_has_payload`, `discriminant_offset`, `discriminant_size`) and
   `AbiTagUnionLayout`; records and tags now carry explicit sizes, alignments and
   offsets per pointer width (`AbiLayout.size32/size64`, `alignment32/alignment64`).
3. Refcountedness is now emitted (`AbiLayout.contains_refcounted`, `HostRcPlan`
   with `RcNoop` / `RcRefcounted`) — this is the ownership information the
   hand-written host currently guesses at with header slots before each data
   pointer.
4. `TypeTable` is a receiver module (`from_list`, `entries`, `type_info`, ...)
   that glue scripts are expected to ask questions through.

The glue platform sources for the pinned compiler ship locally under
`~/.cache/roc/nightly-<version>/src/compiler-platforms/glue/<hash>/`, which is
the reference for the current API.

