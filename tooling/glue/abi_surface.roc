app [make_glue] { pf: platform glue }

import pf.GlueInput exposing [GlueInput]
import pf.File exposing [File]
import pf.Types exposing [Types]

## Minimal glue spec for this platform.
##
## It does not generate ABI code yet: it runs the compiler's glue pipeline
## against `platform/main.roc` and dumps the interface the Rust host has to
## satisfy, which is the input a full Rust generator would consume.
##
## Run from the repository root (ROC nightly pinned in .github/workflows/):
##
##   roc glue tooling/glue/abi_surface.roc /tmp/glue-out platform/main.roc
##   cat /tmp/glue-out/abi_surface.txt
make_glue : List(Types) -> Try(List(File), Str)
make_glue = |types_list| {
        input = GlueInput.from_types(types_list)

        provides_lines = List.map(input.provides_entries, |entry|
                "provides  ${entry.name}  ->  ${entry.ffi_symbol}  (type_id ${U64.to_str(entry.type_id)})")

        hosted_lines = List.map(input.hosted_functions, |func|
                "hosted    ${func.name}  (index ${U64.to_str(func.index)})")

        all_lines = List.concat(provides_lines, hosted_lines)
        content = Str.join_with(all_lines, "\n")

        Ok([{ name: "abi_surface.txt", content }])
}
