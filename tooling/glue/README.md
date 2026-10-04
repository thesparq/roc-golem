# Glue specs

`roc glue` is the compiler-supported way to derive the Rust side of a platform
from the compiler's own ABI knowledge, instead of hand-matching layouts in
`host/src/roc_std.rs` against each nightly.

`abi_surface.roc` is a minimal spec that proves the pipeline runs against this
platform and prints what the host must provide/receive:

```bash
roc glue tooling/glue/abi_surface.roc /tmp/glue-out platform/main.roc
cat /tmp/glue-out/abi_surface.txt
```

Verified output with `nightly-2026-09-19-d025939`:

```
provides  main_init_for_host!  ->  main_init_for_host_1_exposed_generic  (type_id 31)
provides  main_handle_message_for_host!  ->  main_handle_message_for_host_1_exposed_generic  (type_id 34)
provides  main_handle_tool_call_for_host!  ->  main_handle_tool_call_for_host_1_exposed_generic  (type_id 34)
provides  main_metadata_for_host  ->  main_metadata_for_host_1_exposed_generic  (type_id 35)
hosted    Host.rocFxLog!  (index 0)
... 11 hosted effects in total
```

## What is still missing

A full Rust generator (the port of the upstream `RustGlue.roc`) that turns those
type ids into correct Rust structs and call wrappers. The copy of that spec in
the older `roc-practice/roc-golem` project was written against a July 2026 glue
platform and does **not** run on the pinned nightly; porting it needs:

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
