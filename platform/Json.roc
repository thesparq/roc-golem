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

## Escapes special JSON characters (backslashes, quotes, newlines, carriage returns, tabs).
escape : Str -> Str
escape = |input|
	input
		|> Str.replace_each("\\", "\\\\")
		|> Str.replace_each("\"", "\\\"")
		|> Str.replace_each("\n", "\\n")
		|> Str.replace_each("\r", "\\r")
		|> Str.replace_each("\t", "\\t")
