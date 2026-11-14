module [
	encode,
	decode,
	decode_or,
	escape,
]

import Types exposing [Result]

## Serializes any Encodable Roc record, list, or primitive into a valid JSON string.
encode : a -> Str where [a.Json.Encodable([])]
encode = |val|
	Json.to_str(val)

## Deserializes a JSON string into a Parseable Roc type, returning Result(a, Str).
decode : Str -> Result(a, Str) where [a.Json.Parseable([InvalidJson(Str), ..errs])]
decode = |json_str|
	match Json.parse(json_str) {
		Ok(val) => Ok(val)
		Err(InvalidJson(msg)) => Err("Invalid JSON: ${msg}")
		Err(_) => Err("Failed to parse JSON")
	}

## Deserializes a JSON string, or returns a fallback default value on error.
decode_or : Str, a -> a where [a.Json.Parseable([InvalidJson(Str), ..errs])]
decode_or = |json_str, default_val|
	match decode(json_str) {
		Ok(val) => val
		Err(_) => default_val
	}

## Escapes special JSON characters (backslashes, quotes, newlines, carriage returns, tabs, and ASCII control characters U+0000..U+001F).
escape : Str -> Str
escape = |input|
	input
		|> Str.replace_each("\\", "\\\\")
		|> Str.replace_each("\"", "\\\"")
		|> Str.replace_each("\n", "\\n")
		|> Str.replace_each("\r", "\\r")
		|> Str.replace_each("\t", "\\t")
		|> Str.replace_each("\u(0000)", "\\u0000")
		|> Str.replace_each("\u(0001)", "\\u0001")
		|> Str.replace_each("\u(0002)", "\\u0002")
		|> Str.replace_each("\u(0003)", "\\u0003")
		|> Str.replace_each("\u(0004)", "\\u0004")
		|> Str.replace_each("\u(0005)", "\\u0005")
		|> Str.replace_each("\u(0006)", "\\u0006")
		|> Str.replace_each("\u(0007)", "\\u0007")
		|> Str.replace_each("\u(0008)", "\\b")
		|> Str.replace_each("\u(000b)", "\\u000b")
		|> Str.replace_each("\u(000c)", "\\f")
		|> Str.replace_each("\u(000e)", "\\u000e")
		|> Str.replace_each("\u(000f)", "\\u000f")
		|> Str.replace_each("\u(0010)", "\\u0010")
		|> Str.replace_each("\u(0011)", "\\u0011")
		|> Str.replace_each("\u(0012)", "\\u0012")
		|> Str.replace_each("\u(0013)", "\\u0013")
		|> Str.replace_each("\u(0014)", "\\u0014")
		|> Str.replace_each("\u(0015)", "\\u0015")
		|> Str.replace_each("\u(0016)", "\\u0016")
		|> Str.replace_each("\u(0017)", "\\u0017")
		|> Str.replace_each("\u(0018)", "\\u0018")
		|> Str.replace_each("\u(0019)", "\\u0019")
		|> Str.replace_each("\u(001a)", "\\u001a")
		|> Str.replace_each("\u(001b)", "\\u001b")
		|> Str.replace_each("\u(001c)", "\\u001c")
		|> Str.replace_each("\u(001d)", "\\u001d")
		|> Str.replace_each("\u(001e)", "\\u001e")
		|> Str.replace_each("\u(001f)", "\\u001f")
