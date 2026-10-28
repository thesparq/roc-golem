# Glue specs

`roc glue` is the compiler's official way to generate a platform host's ABI
layer: a glue *spec* (a Roc app against the `glue` platform) receives the
compiler-emitted type table and writes host-language source. The compiler repo
ships `src/glue/src/RustGlue.roc` for Rust hosts, and its README is explicit
that platform code should consume generated glue rather than hand-rolling ABI
bindings, because the layout, ownership and refcounting contracts are compiler
internals.

## The official Rust glue (adopted)

`RustGlue.roc` runs against this platform and writes one Rust file, which is
checked in as `host/src/roc_platform_abi.rs` and compiled by the host crate:

```bash
# fetch the spec from the commit the pinned nightly was built from. The nightly
# tag carries its commit (`nightly-2026-10-27-de79a0f` -> de79a0f...); resolve
# it with `gh api repos/roc-lang/roc/commits/de79a0f --jq .sha` if needed.
gh api "repos/roc-lang/roc/contents/src/glue/src/RustGlue.roc?ref=<nightly-commit>" \
  --jq .content | base64 -d > /tmp/RustGlue.roc
roc glue /tmp/RustGlue.roc /tmp/glue-out platform/main.roc
cp /tmp/glue-out/roc_platform_abi.rs host/src/roc_platform_abi.rs
```

(The `roc_platform_abi.rs` name is the generator's; the older copy of this
README pointed at a `tooling/glue/generated/` directory that does not exist in
this repo.)

Regenerating after an effect, an entrypoint or a nightly change is the whole
procedure — the file is generated, not edited. It contains:

- `RocStr`, `RocListWith<T, ELEMENTS_REFCOUNTED>` (with the `RocList<T>` alias),
  `RocDec` and the rest of the ABI-visible types, with the layout documented in
  the same terms this repo reverse-engineered by hand (capacity shifted left by
  one, seamless-slice tag in the capacity word, refcount one word before the
  bytes);
- retain/release plans (`RocStrRelease`, `RocListRelease`, `RocListSpineRelease`,
  `RocOwned`), including the recursive teardown rules;
- every **hosted symbol** the host must export (`rocFx*` — 83 of them today) with
  the exact direct C ABI signatures and per-argument ownership notes;
- a **shell per result shape** (`HostRocFxRpcInvokeResult` for `Try(Str, Str)`,
  `HostRocFxFileSizeResult` for `Try(U64, Str)`, ...) with borrow/take accessors
  for each payload and a release type, deduplicated by shape and aliased by
  symbol (`HostRocFxWsSendResult = HostRocFxAppendFileResult`);
- **owning wrappers for the provided entrypoints** (`main_*_owned`) plus
  `const _: () = assert!(...)` size/alignment checks.

### What stays hand-written, and why

`host/src/roc_std.rs` is now small on purpose. The generated shells have
accessors and release plans but **no constructors** — and the host *constructs* a
result for every effect it answers (181 `RocResult` sites and 7 `RocU64Result`
ones). Migrating those to the generated types would mean writing the payload
bytes by hand at each site, so `RocResult<T, E>` and `RocU64Result` stay, and a
`const _: () = assert!(...)` block ties both to the generated shells
(`HostRocFxRpcInvokeResult`, `HostRocFxFileSizeResult`) by size, alignment and
discriminant offset on every target — a layout change on either side fails the
build. The unused hand-written `RocList` is gone; the generated `RocList<T>`
alias is the list type if the host ever needs one.

The entrypoint calls in `host/src/lib.rs` still use `MaybeUninit` +
`mem::forget` rather than the generated `main_*_owned` wrappers, and the host
still defines the allocator exports (`roc_alloc`/`roc_dealloc`/`roc_realloc`,
`cabi_realloc`) itself — the generated file declares them as externs with the
contract spelled out. Switching the calls over is the remaining step if the two
ever diverge; the wrappers are compiled in the meantime, so they cannot rot.

## The diagnostic specs

The two specs below dump what *this* platform declares, which is how the
`host/src/roc_std.rs` layout constants were derived and how a nightly bump was
verified before the official generator was available:

```bash
roc glue tooling/glue/abi_facts.roc   /tmp/glue-out platform/main.roc
cat /tmp/glue-out/abi_facts.txt
```

They run on `nightly-2026-09-19-d025939` and **not** on the pinned
`nightly-2026-10-27-de79a0f`: the glue platform's API moved (`pf.FunctionRepr`
is gone; `pf.FunctionInfo`, `pf.FunctionSignature`, `pf.ProvidedExport` and
`pf.HostedFunctionInfo` replaced it) and the new compiler's type-module
convention rejects the `import pf.X exposing [X]` clauses these specs use. They
stay for the table below, which records what was reverse-engineered before the
official generator existed; porting them is optional now that the generator
emits the layout assertions and ownership rules itself.

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

Nothing in the generator's path: `RustGlue.roc` runs on the pinned nightly and is
what produced the checked-in `host/src/roc_platform_abi.rs` (see the first
section). What is still *hand-written* is listed there — the result shells' 
constructors and the entrypoint call sites — and the older port of this spec in
`roc-practice/roc-golem` is no longer needed for anything; it was written against
a July 2026 glue platform and would have to be brought up to the current API
(`TypeRepr` variants, the `AbiLayout` / `AbiTagUnionLayout` moves, the `HostRcPlan`
refcountedness facts, `TypeTable` as a receiver module) for no purpose.

The glue platform sources for the installed compiler ship locally under
`~/.cache/roc/<version>/src/compiler-platforms/glue/<hash>/`, which is the
reference for the API a glue script runs against.

