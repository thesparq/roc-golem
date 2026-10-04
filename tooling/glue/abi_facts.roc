app [make_glue] { pf: platform glue }

import pf.GlueInput exposing [GlueInput]
import pf.File exposing [File]
import pf.Types exposing [Types]
import pf.TypeInfo exposing [TypeInfo]
import pf.TypeRepr exposing [TypeRepr]
import pf.AbiLayout exposing [AbiLayout]
import pf.AbiLayoutDetails exposing [AbiLayoutDetails]
import pf.AbiTagLayout exposing [AbiTagLayout]
import pf.AbiFieldLayout exposing [AbiFieldLayout]
import pf.HostRcPlan exposing [HostRcPlan]
import pf.TagUnionRepr exposing [TagUnionRepr]
import pf.RecordRepr exposing [RecordRepr]
import pf.FunctionRepr exposing [FunctionRepr]

## Dump every ABI fact the compiler emits for this platform: the host-facing
## symbols, and for each type id the reflected shape plus the committed layout.
make_glue : List(Types) -> Try(List(File), Str)
make_glue = |types_list| {
        input = GlueInput.from_types(types_list)

        var $lines = []
        $lines = $lines.append("== provides (host calls into the app) ==")
        for entry in input.provides_entries {
                $lines = $lines.append("  ${entry.name}  symbol=${entry.ffi_symbol}  type_id=${U64.to_str(entry.type_id)}")
        }

        $lines = $lines.append("")
        $lines = $lines.append("== hosted (app calls into the host) ==")
        for func in input.hosted_functions {
                $lines = $lines.append("  #${U64.to_str(func.index)} ${func.name}  symbol=${func.ffi_symbol}")
                $lines = $lines.append("      type=${func.type_str}")
                $lines = $lines.append("      arg_ids=[${join_ids(func.arg_type_ids)}] ret_id=${U64.to_str(func.ret_type_id)}")
        }

        $lines = $lines.append("")
        $lines = $lines.append("== types ==")
        var $index = 0
        for info in input.types {
                $lines = $lines.append("#${U64.to_str($index)}  ${render_repr(info.repr)}  rc=${render_rc(info.rc)}  contains_refcounted=${bool_str(info.layout.contains_refcounted)}")
                $lines = $lines.append("    size32=${U64.to_str(info.layout.size32)} size64=${U64.to_str(info.layout.size64)} align32=${U64.to_str(info.layout.alignment32)} align64=${U64.to_str(info.layout.alignment64)}")
                $lines = $lines.append(render_details(info.layout.details))
                $index = $index + 1
        }

        Ok([{ name: "abi_facts.txt", content: Str.join_with($lines, "\n") }])
}

bool_str : Bool -> Str
bool_str = |b| if b "true" else "false"

join_ids : List(U64) -> Str
join_ids = |ids| Str.join_with(List.map(ids, U64.to_str), ",")

render_rc : HostRcPlan -> Str
render_rc = |rc|
        match rc {
                RcNoop => "RcNoop"
                RcRefcounted => "RcRefcounted"
        }

render_repr : TypeRepr -> Str
render_repr = |repr|
        match repr {
                RocStr => "Str"
                RocU8 => "U8"
                RocU32 => "U32"
                RocU64 => "U64"
                RocBool => "Bool"
                RocUnit => "Unit"
                RocDec => "Dec"
                RocList(inner) => "List(#${U64.to_str(inner)})"
                RocBox(inner) => "Box(#${U64.to_str(inner)})"
                RocRecord(record) => "Record(${render_record(record)})"
                RocTagUnion(tu) => "TagUnion(${render_tag_union(tu)})"
                RocFunction(f) => "Fn(args=[${join_ids(f.args)}] -> ${U64.to_str(f.ret)})"
                _ => "other"
        }

render_record : RecordRepr -> Str
render_record = |record|
        if record.name == "" "anonymous" else record.name

render_tag_union : TagUnionRepr -> Str
render_tag_union = |tu| {
        names = Str.join_with(List.map(tu.tags, |tag| tag.name), "|")
        if tu.name == "" "${names}" else "${tu.name}:${names}"
}

render_details : AbiLayoutDetails -> Str
render_details = |details|
        match details {
                AbiBuiltin => "    details: builtin"
                AbiRecord(record) => "    record fields:\n${render_fields(record.fields)}"
                AbiTagUnion(tu) => "    tag union: disc_offset32=${U64.to_str(tu.discriminant_offset32)} disc_offset64=${U64.to_str(tu.discriminant_offset64)} disc_size=${U64.to_str(tu.discriminant_size)} has_payload=${bool_str(tu.has_payload)}\n${render_tags(tu.tags)}"
        }

render_fields : List(AbiFieldLayout) -> Str
render_fields = |fields|
        Str.join_with(List.map(fields, |f|
                "      ${f.name}  type_id=${U64.to_str(f.type_id)}  off32=${U64.to_str(f.offset32)}  off64=${U64.to_str(f.offset64)}  size32=${U64.to_str(f.size32)}  size64=${U64.to_str(f.size64)}  padding=${bool_str(f.is_padding)}"), "\n")

render_tags : List(AbiTagLayout) -> Str
render_tags = |tags|
        Str.join_with(List.map(tags, |t|
                "      ${t.name}  disc=${U64.to_str(t.discriminant)}  payload_size32=${U64.to_str(t.payload_size32)}  payload_size64=${U64.to_str(t.payload_size64)}  payload_fields:\n${render_fields(t.payload_fields)}"), "\n")
