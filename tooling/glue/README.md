# Glue specs

`roc glue` is the compiler-supported way to derive the Rust side of a platform
from the compiler's own ABI knowledge, instead of hand-matching layouts in
`host/src/roc_std.rs` against each nightly.

Two specs live here, both of which run on the pinned nightly
(`nightly-2026-09-19-d025939`):

```bash
roc glue tooling/glue/abi_surface.roc /tmp/glue-out platform/main.roc
roc glue tooling/glue/abi_facts.roc   /tmp/glue-out platform/main.roc
cat /tmp/glue-out/abi_surface.txt /tmp/glue-out/abi_facts.txt
```

- `abi_surface.roc` lists the host-facing symbols (what the host calls, and what
  the app calls back into).
- `abi_facts.roc` additionally dumps the committed layout of every type the
  platform mentions: sizes, alignments, field offsets, tag discriminants and
  refcountedness.

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
stores there — is not part of the emitted facts. Those three functions only serve
the Roc runtime; the component-model boundary uses `cabi_realloc`, which
wit-bindgen's glue frees through Rust's global allocator and which therefore must
be a plain allocation with no Roc header. Mixing the two is what produced the
long-lived `initialize` trap (see the Development status section of the root
README).

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

