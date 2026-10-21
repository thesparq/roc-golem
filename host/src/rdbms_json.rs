//! The JSON mirror of the three `golem:rdbms` `db-value` vocabularies.
//!
//! Each engine's interface models a database value as its own variant
//! (`golem:rdbms/postgres.db-value`, `…/mysql.db-value`, `…/ignite2.db-value`)
//! and a result as `{columns, rows}`. Roc has no variants to hand such a value
//! to an app, so — the way `http!` hands over a structured response and
//! `retry_json` mirrors the retry trees — the platform renders a value as JSON
//! text and parses the same shape back. The text is the WIT structure, not a
//! translation of it:
//!
//! * a value is `{"tag": …, "value": …}`; the WIT case names and record field
//!   names appear in camelCase (the platform's convention for WIT names, as in
//!   `retry_json`), and a case that carries no payload has just its tag
//!   (`{"tag": "null"}`),
//! * a record is a JSON object with its fields, a list or tuple is a JSON array,
//!   and numbers, strings and booleans are themselves,
//! * a nested variant (`ip-address`, a range bound) is a tagged object in the
//!   same shape,
//! * a `lazy-db-value` / `lazy-db-column-type` is what it wraps,
//! * the hand-mapped cases: a `uuid` (and Ignite's `db-uuid`) is the canonical
//!   UUID string, and a non-finite float is `"NaN"` / `"Infinity"` /
//!   `"-Infinity"` — JSON has no number for those — which parses back to the
//!   same float.
//!
//! A `db-result` renders as `{"columns": […], "rows": […]}`: a column is the
//! WIT `db-column` (`{"ordinal": …, "name": …, "dbType": …, "dbTypeName": …}`,
//! except on Ignite, whose column carries no type), and a row is the JSON array
//! of its values in column order — `db-row`'s single `values` field is that
//! array. A `db-column-type` is a tagged object like a value
//! (`{"tag": "int4"}`), since it too is a WIT variant.
//!
//! The tag sets differ per engine because the WIT vocabularies do: postgres
//! `int4` and `text` where MySQL has `int` and `varchar` and Ignite has
//! `dbInt` and `dbString`. An app knows which engine it opened, so a value's
//! tags are the vocabulary of that engine — the README's database section lists
//! the common ones.

use serde_json::{json, Map, Value};

use crate::golem::rdbms::types::{
    Date, IpAddress, MacAddress, Time, Timestamp, Timestamptz, Timetz, Uuid as DbUuid,
};
use crate::golem::rdbms::{ignite2, mysql, postgres};

// The canonical UUID text
//
// Two of the three vocabularies carry a UUID as a pair of words (postgres as a
// `uuid` record, Ignite as a `(u64, u64)` tuple), and the platform's other
// effects already hand UUIDs to apps as their canonical text (`fork!`,
// `idempotencyKey!`, `resolveComponentId!`), so a UUID is the one value that is
// not mirrored structurally.

/// The canonical hyphenated spelling of a two-word UUID.
pub fn uuid_text(high: u64, low: u64) -> String {
    format!(
        "{:08x}-{:04x}-{:04x}-{:04x}-{:012x}",
        high >> 32,
        (high >> 16) & 0xffff,
        high & 0xffff,
        low >> 48,
        low & 0xffff_ffff_ffff,
    )
}

/// Parses that spelling back into its two words (dashes optional).
pub fn uuid_bits(text: &str) -> Result<(u64, u64), String> {
    let hex: String = text.chars().filter(|c| *c != '-').collect();
    let not_a_uuid = || format!("the value {text:?} is not a UUID");
    if hex.len() != 32 || !hex.chars().all(|c| c.is_ascii_hexdigit()) {
        return Err(not_a_uuid());
    }
    let high = u64::from_str_radix(&hex[..16], 16).map_err(|_| not_a_uuid())?;
    let low = u64::from_str_radix(&hex[16..], 16).map_err(|_| not_a_uuid())?;
    Ok((high, low))
}

// The params of `dbQuery!` / `dbExecute!`

/// Parses the params text into its list of encoded values. The top level is a
/// JSON array (empty for a statement without parameters), and each element is
/// decoded by the engine's own `*_params` below.
pub fn params_array(text: &str) -> Result<Vec<Value>, String> {
    let value: Value = serde_json::from_str(text)
        .map_err(|err| format!("the params are not valid JSON: {err}"))?;
    match value {
        Value::Array(items) => Ok(items),
        other => Err(format!(
            "the params must be a JSON array of db-values, got {}",
            json_kind(&other)
        )),
    }
}

/// The JSON kind of a value, for the error messages a malformed encoding gets.
fn json_kind(value: &Value) -> &'static str {
    match value {
        Value::Null => "null",
        Value::Bool(_) => "a boolean",
        Value::Number(_) => "a number",
        Value::String(_) => "a string",
        Value::Array(_) => "an array",
        Value::Object(_) => "an object",
    }
}

// The tagged-object plumbing

/// The `(tag, payload)` of one encoded value.
fn tagged(value: &Value) -> Result<(&str, Option<&Value>), String> {
    let Some(object) = value.as_object() else {
        return Err(format!(
            "a db-value must be a JSON object like {{\"tag\":\"int4\",\"value\":1}}, got {}",
            json_kind(value)
        ));
    };
    let Some(tag) = object.get("tag").and_then(Value::as_str) else {
        return Err(String::from("a db-value needs a string \"tag\""));
    };
    Ok((tag, object.get("value")))
}

fn needs_payload<'a>(tag: &str, payload: Option<&'a Value>) -> Result<&'a Value, String> {
    payload.ok_or_else(|| format!("the db-value \"{tag}\" needs a \"value\""))
}

fn needs_no_payload(tag: &str, payload: Option<&Value>) -> Result<(), String> {
    match payload {
        None => Ok(()),
        Some(_) => Err(format!("the db-value \"{tag}\" takes no \"value\"")),
    }
}

fn object_payload<'a>(
    tag: &str,
    payload: Option<&'a Value>,
) -> Result<&'a Map<String, Value>, String> {
    needs_payload(tag, payload)?
        .as_object()
        .ok_or_else(|| format!("the db-value \"{tag}\" needs an object \"value\""))
}

fn array_payload<'a>(tag: &str, payload: Option<&'a Value>) -> Result<&'a Vec<Value>, String> {
    needs_payload(tag, payload)?
        .as_array()
        .ok_or_else(|| format!("the db-value \"{tag}\" needs an array \"value\""))
}

// Payload readers: the hand-mapped common cases

fn int_payload(tag: &str, payload: Option<&Value>) -> Result<i64, String> {
    needs_payload(tag, payload)?
        .as_i64()
        .ok_or_else(|| format!("the db-value \"{tag}\" needs an integer \"value\""))
}

fn uint_payload(tag: &str, payload: Option<&Value>) -> Result<u64, String> {
    needs_payload(tag, payload)?
        .as_u64()
        .ok_or_else(|| format!("the db-value \"{tag}\" needs a non-negative integer \"value\""))
}

fn i8_payload(tag: &str, payload: Option<&Value>) -> Result<i8, String> {
    i8::try_from(int_payload(tag, payload)?)
        .map_err(|_| format!("the db-value \"{tag}\" needs an 8-bit number"))
}

fn i16_payload(tag: &str, payload: Option<&Value>) -> Result<i16, String> {
    i16::try_from(int_payload(tag, payload)?)
        .map_err(|_| format!("the db-value \"{tag}\" needs a 16-bit number"))
}

fn i32_payload(tag: &str, payload: Option<&Value>) -> Result<i32, String> {
    i32::try_from(int_payload(tag, payload)?)
        .map_err(|_| format!("the db-value \"{tag}\" needs a 32-bit number"))
}

fn u16_payload(tag: &str, payload: Option<&Value>) -> Result<u16, String> {
    u16::try_from(uint_payload(tag, payload)?)
        .map_err(|_| format!("the db-value \"{tag}\" needs a 16-bit number"))
}

fn u32_payload(tag: &str, payload: Option<&Value>) -> Result<u32, String> {
    u32::try_from(uint_payload(tag, payload)?)
        .map_err(|_| format!("the db-value \"{tag}\" needs a 32-bit number"))
}

fn bool_payload(tag: &str, payload: Option<&Value>) -> Result<bool, String> {
    needs_payload(tag, payload)?
        .as_bool()
        .ok_or_else(|| format!("the db-value \"{tag}\" needs a boolean \"value\""))
}

fn text_payload(tag: &str, payload: Option<&Value>) -> Result<String, String> {
    needs_payload(tag, payload)?
        .as_str()
        .map(str::to_string)
        .ok_or_else(|| format!("the db-value \"{tag}\" needs a string \"value\""))
}

/// A float payload: a JSON number, or one of the three spellings JSON cannot
/// carry as a number.
fn float_payload(tag: &str, payload: Option<&Value>) -> Result<f64, String> {
    let payload = needs_payload(tag, payload)?;
    if let Some(number) = payload.as_f64() {
        return Ok(number);
    }
    match payload.as_str() {
        Some("NaN") => Ok(f64::NAN),
        Some("Infinity") => Ok(f64::INFINITY),
        Some("-Infinity") => Ok(f64::NEG_INFINITY),
        _ => Err(format!(
            "the db-value \"{tag}\" needs a number \"value\" (or \"NaN\", \"Infinity\", \"-Infinity\")"
        )),
    }
}

fn bytes_payload(tag: &str, payload: Option<&Value>) -> Result<Vec<u8>, String> {
    array_payload(tag, payload)?
        .iter()
        .map(|byte| {
            byte.as_u64()
                .and_then(|byte| u8::try_from(byte).ok())
                .ok_or_else(|| {
                    format!("the db-value \"{tag}\" needs an array of bytes (0..=255)")
                })
        })
        .collect()
}

fn bits_payload(tag: &str, payload: Option<&Value>) -> Result<Vec<bool>, String> {
    array_payload(tag, payload)?
        .iter()
        .map(|bit| {
            bit.as_bool()
                .ok_or_else(|| format!("the db-value \"{tag}\" needs an array of booleans"))
        })
        .collect()
}

fn floats_payload(tag: &str, payload: Option<&Value>) -> Result<Vec<f32>, String> {
    array_payload(tag, payload)?
        .iter()
        .map(|number| {
            number
                .as_f64()
                .map(|number| number as f32)
                .ok_or_else(|| format!("the db-value \"{tag}\" needs an array of numbers"))
        })
        .collect()
}

fn uuid_payload(tag: &str, payload: Option<&Value>) -> Result<DbUuid, String> {
    let (high, low) = uuid_bits(&text_payload(tag, payload)?)?;
    Ok(DbUuid {
        high_bits: high,
        low_bits: low,
    })
}

// The `golem:rdbms/types` records, shared by postgres and MySQL

fn date_payload(tag: &str, payload: Option<&Value>) -> Result<Date, String> {
    let object = object_payload(tag, payload)?;
    Ok(Date {
        year: i32_field(object, tag, "year")?,
        month: u8_field(object, tag, "month")?,
        day: u8_field(object, tag, "day")?,
    })
}

fn time_payload(tag: &str, payload: Option<&Value>) -> Result<Time, String> {
    let object = object_payload(tag, payload)?;
    Ok(Time {
        hour: u8_field(object, tag, "hour")?,
        minute: u8_field(object, tag, "minute")?,
        second: u8_field(object, tag, "second")?,
        nanosecond: u32_field(object, tag, "nanosecond")?,
    })
}

fn timestamp_payload(tag: &str, payload: Option<&Value>) -> Result<Timestamp, String> {
    let object = object_payload(tag, payload)?;
    Ok(Timestamp {
        date: date_payload("date", object.get("date"))?,
        time: time_payload("time", object.get("time"))?,
    })
}

fn timestamptz_payload(tag: &str, payload: Option<&Value>) -> Result<Timestamptz, String> {
    let object = object_payload(tag, payload)?;
    Ok(Timestamptz {
        timestamp: timestamp_payload("timestamp", object.get("timestamp"))?,
        offset: i32_field(object, tag, "offset")?,
    })
}

fn timetz_payload(tag: &str, payload: Option<&Value>) -> Result<Timetz, String> {
    let object = object_payload(tag, payload)?;
    Ok(Timetz {
        time: time_payload("time", object.get("time"))?,
        offset: i32_field(object, tag, "offset")?,
    })
}

fn ip_address_payload(tag: &str, payload: Option<&Value>) -> Result<IpAddress, String> {
    let (nested, nested_payload) = tagged(needs_payload(tag, payload)?)?;
    match nested {
        "ipv4" => Ok(IpAddress::Ipv4(ipv4_payload(tag, nested_payload)?)),
        "ipv6" => Ok(IpAddress::Ipv6(ipv6_payload(tag, nested_payload)?)),
        other => Err(format!(
            "the db-value \"{tag}\" takes an ip-address, and \"{other}\" is no ip-address case"
        )),
    }
}

fn ipv4_payload(tag: &str, payload: Option<&Value>) -> Result<(u8, u8, u8, u8), String> {
    let bytes = bytes_payload(tag, payload)?;
    let Ok([a, b, c, d]) = <[u8; 4]>::try_from(bytes.as_slice()) else {
        return Err(format!("an ipv4 address is four bytes, got {}", bytes.len()));
    };
    Ok((a, b, c, d))
}

fn ipv6_payload(tag: &str, payload: Option<&Value>) -> Result<(u16, u16, u16, u16, u16, u16, u16, u16), String> {
    let segments = array_payload(tag, payload)?;
    let mut words = [0u16; 8];
    if segments.len() != 8 {
        return Err(format!(
            "an ipv6 address is eight words, got {}",
            segments.len()
        ));
    }
    for (word, segment) in words.iter_mut().zip(segments) {
        *word = u16::try_from(segment.as_u64().ok_or_else(|| {
            String::from("an ipv6 address is eight numbers")
        })?)
        .map_err(|_| String::from("an ipv6 word is 0..=65535"))?;
    }
    Ok((
        words[0], words[1], words[2], words[3], words[4], words[5], words[6], words[7],
    ))
}

fn mac_address_payload(tag: &str, payload: Option<&Value>) -> Result<MacAddress, String> {
    let object = object_payload(tag, payload)?;
    let octets = bytes_payload("octets", object.get("octets"))?;
    let Ok([a, b, c, d, e, f]) = <[u8; 6]>::try_from(octets.as_slice()) else {
        return Err(format!(
            "a mac address is six octets, got {}",
            octets.len()
        ));
    };
    Ok(MacAddress {
        octets: (a, b, c, d, e, f),
    })
}

fn u8_field(object: &Map<String, Value>, tag: &str, name: &str) -> Result<u8, String> {
    u8::try_from(int_field(object, tag, name)?)
        .map_err(|_| format!("the db-value \"{tag}\" needs \"{name}\" as a byte"))
}

fn u32_field(object: &Map<String, Value>, tag: &str, name: &str) -> Result<u32, String> {
    u32::try_from(int_field(object, tag, name)?)
        .map_err(|_| format!("the db-value \"{tag}\" needs \"{name}\" as a 32-bit number"))
}

fn i32_field(object: &Map<String, Value>, tag: &str, name: &str) -> Result<i32, String> {
    i32::try_from(int_field(object, tag, name)?)
        .map_err(|_| format!("the db-value \"{tag}\" needs \"{name}\" as a 32-bit number"))
}

fn int_field(object: &Map<String, Value>, tag: &str, name: &str) -> Result<i64, String> {
    object
        .get(name)
        .and_then(Value::as_i64)
        .ok_or_else(|| format!("the db-value \"{tag}\" needs an integer \"{name}\""))
}

// The render side: shared payload shapes

fn date_json(date: &Date) -> Value {
    json!({ "year": date.year, "month": date.month, "day": date.day })
}

fn time_json(time: &Time) -> Value {
    json!({
        "hour": time.hour,
        "minute": time.minute,
        "second": time.second,
        "nanosecond": time.nanosecond,
    })
}

fn timestamp_json(timestamp: &Timestamp) -> Value {
    json!({ "date": date_json(&timestamp.date), "time": time_json(&timestamp.time) })
}

fn timestamptz_json(value: &Timestamptz) -> Value {
    json!({
        "timestamp": timestamp_json(&value.timestamp),
        "offset": value.offset,
    })
}

fn timetz_json(value: &Timetz) -> Value {
    json!({ "time": time_json(&value.time), "offset": value.offset })
}

/// A float as JSON: a number, or the text JSON has no number for.
fn float_json(number: f64) -> Value {
    if number.is_nan() {
        json!("NaN")
    } else if number.is_infinite() {
        json!(if number > 0.0 { "Infinity" } else { "-Infinity" })
    } else {
        json!(number)
    }
}

fn float32_json(number: f32) -> Value {
    float_json(number as f64)
}

fn ip_address_json(address: &IpAddress) -> Value {
    match address {
        IpAddress::Ipv4((a, b, c, d)) => json!({ "tag": "ipv4", "value": [a, b, c, d] }),
        IpAddress::Ipv6((a, b, c, d, e, f, g, h)) => {
            json!({ "tag": "ipv6", "value": [a, b, c, d, e, f, g, h] })
        }
    }
}

fn mac_address_json(address: &MacAddress) -> Value {
    let (a, b, c, d, e, f) = address.octets;
    json!({ "octets": [a, b, c, d, e, f] })
}

// ------------------------------------------------------------------ postgres

/// The postgres params: each element decoded into `golem:rdbms/postgres`'s
/// `db-value`.
pub fn postgres_params(items: &[Value]) -> Result<Vec<postgres::DbValue>, String> {
    items.iter().map(postgres_value).collect()
}

fn postgres_value(value: &Value) -> Result<postgres::DbValue, String> {
    let (tag, payload) = tagged(value)?;
    Ok(match tag {
        "character" => postgres::DbValue::Character(i8_payload(tag, payload)?),
        "int2" => postgres::DbValue::Int2(i16_payload(tag, payload)?),
        "int4" => postgres::DbValue::Int4(i32_payload(tag, payload)?),
        "int8" => postgres::DbValue::Int8(int_payload(tag, payload)?),
        "float4" => postgres::DbValue::Float4(float_payload(tag, payload)? as f32),
        "float8" => postgres::DbValue::Float8(float_payload(tag, payload)?),
        "numeric" => postgres::DbValue::Numeric(text_payload(tag, payload)?),
        "boolean" => postgres::DbValue::Boolean(bool_payload(tag, payload)?),
        "text" => postgres::DbValue::Text(text_payload(tag, payload)?),
        "varchar" => postgres::DbValue::Varchar(text_payload(tag, payload)?),
        "bpchar" => postgres::DbValue::Bpchar(text_payload(tag, payload)?),
        "timestamp" => postgres::DbValue::Timestamp(timestamp_payload(tag, payload)?),
        "timestamptz" => postgres::DbValue::Timestamptz(timestamptz_payload(tag, payload)?),
        "date" => postgres::DbValue::Date(date_payload(tag, payload)?),
        "time" => postgres::DbValue::Time(time_payload(tag, payload)?),
        "timetz" => postgres::DbValue::Timetz(timetz_payload(tag, payload)?),
        "interval" => postgres::DbValue::Interval(postgres_interval(tag, payload)?),
        "bytea" => postgres::DbValue::Bytea(bytes_payload(tag, payload)?),
        "json" => postgres::DbValue::Json(text_payload(tag, payload)?),
        "jsonb" => postgres::DbValue::Jsonb(text_payload(tag, payload)?),
        "jsonpath" => postgres::DbValue::Jsonpath(text_payload(tag, payload)?),
        "xml" => postgres::DbValue::Xml(text_payload(tag, payload)?),
        "uuid" => postgres::DbValue::Uuid(uuid_payload(tag, payload)?),
        "inet" => postgres::DbValue::Inet(ip_address_payload(tag, payload)?),
        "cidr" => postgres::DbValue::Cidr(ip_address_payload(tag, payload)?),
        "macaddr" => postgres::DbValue::Macaddr(mac_address_payload(tag, payload)?),
        "bit" => postgres::DbValue::Bit(bits_payload(tag, payload)?),
        "varbit" => postgres::DbValue::Varbit(bits_payload(tag, payload)?),
        "int4range" => postgres::DbValue::Int4range(postgres_int4range(tag, payload)?),
        "int8range" => postgres::DbValue::Int8range(postgres_int8range(tag, payload)?),
        "numrange" => postgres::DbValue::Numrange(postgres_numrange(tag, payload)?),
        "tsrange" => postgres::DbValue::Tsrange(postgres_tsrange(tag, payload)?),
        "tstzrange" => postgres::DbValue::Tstzrange(postgres_tstzrange(tag, payload)?),
        "daterange" => postgres::DbValue::Daterange(postgres_daterange(tag, payload)?),
        "money" => postgres::DbValue::Money(int_payload(tag, payload)?),
        "oid" => postgres::DbValue::Oid(u32_payload(tag, payload)?),
        "enumeration" => {
            let object = object_payload(tag, payload)?;
            postgres::DbValue::Enumeration(postgres::Enumeration {
                name: text_field(object, tag, "name")?,
                value: text_field(object, tag, "value")?,
            })
        }
        "composite" => {
            let object = object_payload(tag, payload)?;
            postgres::DbValue::Composite(postgres::Composite {
                name: text_field(object, tag, "name")?,
                values: lazy_values("values", object.get("values"))?,
            })
        }
        "domain" => {
            let object = object_payload(tag, payload)?;
            postgres::DbValue::Domain(postgres::Domain {
                name: text_field(object, tag, "name")?,
                value: lazy_value("value", object.get("value"))?,
            })
        }
        "array" => postgres::DbValue::Array(lazy_values(tag, payload)?),
        "range" => {
            let object = object_payload(tag, payload)?;
            postgres::DbValue::Range(postgres::Range {
                name: text_field(object, tag, "name")?,
                value: postgres_values_range("value", object.get("value"))?,
            })
        }
        "null" => {
            needs_no_payload(tag, payload)?;
            postgres::DbValue::Null
        }
        "vector" => postgres::DbValue::Vector(floats_payload(tag, payload)?),
        "halfvec" => postgres::DbValue::Halfvec(floats_payload(tag, payload)?),
        "sparsevec" => {
            let object = object_payload(tag, payload)?;
            postgres::DbValue::Sparsevec(postgres::SparseVec {
                dim: i32_field(object, tag, "dim")?,
                indices: array_payload("indices", object.get("indices"))?
                    .iter()
                    .map(|index| {
                        index.as_i64().and_then(|index| i32::try_from(index).ok()).ok_or_else(
                            || String::from("the db-value \"sparsevec\" needs integer \"indices\""),
                        )
                    })
                    .collect::<Result<Vec<_>, _>>()?,
                values: floats_payload("values", object.get("values"))?,
            })
        }
        other => {
            return Err(format!(
                "unknown db-value tag \"{other}\" for a postgres connection"
            ))
        }
    })
}

fn text_field(object: &Map<String, Value>, tag: &str, name: &str) -> Result<String, String> {
    object
        .get(name)
        .and_then(Value::as_str)
        .map(str::to_string)
        .ok_or_else(|| format!("the db-value \"{tag}\" needs a string \"{name}\""))
}

fn postgres_interval(tag: &str, payload: Option<&Value>) -> Result<postgres::Interval, String> {
    let object = object_payload(tag, payload)?;
    Ok(postgres::Interval {
        months: i32_field(object, tag, "months")?,
        days: i32_field(object, tag, "days")?,
        microseconds: int_field(object, tag, "microseconds")?,
    })
}

/// A `lazy-db-value` payload is what it wraps: the same tagged object.
fn lazy_value(name: &str, payload: Option<&Value>) -> Result<postgres::LazyDbValue, String> {
    Ok(postgres::LazyDbValue::new(postgres_value(needs_payload(
        name, payload,
    )?)?))
}

fn lazy_values(name: &str, payload: Option<&Value>) -> Result<Vec<postgres::LazyDbValue>, String> {
    array_payload(name, payload)?.iter().map(postgres_value).map(|value| value.map(postgres::LazyDbValue::new)).collect()
}

fn postgres_values_range(name: &str, payload: Option<&Value>) -> Result<postgres::ValuesRange, String> {
    let object = object_payload(name, payload)?;
    Ok(postgres::ValuesRange {
        start: postgres_value_bound("start", object.get("start"))?,
        end: postgres_value_bound("end", object.get("end"))?,
    })
}

fn postgres_value_bound(name: &str, payload: Option<&Value>) -> Result<postgres::ValueBound, String> {
    let (tag, bound_payload) = tagged(needs_payload(name, payload)?)?;
    Ok(match tag {
        "included" => postgres::ValueBound::Included(lazy_value(name, bound_payload)?),
        "excluded" => postgres::ValueBound::Excluded(lazy_value(name, bound_payload)?),
        "unbounded" => postgres::ValueBound::Unbounded,
        other => {
            return Err(format!(
                "a range bound is included, excluded or unbounded, and \"{other}\" is none of them"
            ))
        }
    })
}

fn postgres_int4range(tag: &str, payload: Option<&Value>) -> Result<postgres::Int4range, String> {
    let object = object_payload(tag, payload)?;
    Ok(postgres::Int4range {
        start: postgres_int4bound("start", object.get("start"))?,
        end: postgres_int4bound("end", object.get("end"))?,
    })
}

fn postgres_int4bound(name: &str, payload: Option<&Value>) -> Result<postgres::Int4bound, String> {
    let (tag, payload) = tagged(needs_payload(name, payload)?)?;
    Ok(match tag {
        "included" => postgres::Int4bound::Included(i32_payload(name, payload)?),
        "excluded" => postgres::Int4bound::Excluded(i32_payload(name, payload)?),
        "unbounded" => postgres::Int4bound::Unbounded,
        other => return Err(bound_error(other)),
    })
}

fn postgres_int8range(tag: &str, payload: Option<&Value>) -> Result<postgres::Int8range, String> {
    let object = object_payload(tag, payload)?;
    Ok(postgres::Int8range {
        start: postgres_int8bound("start", object.get("start"))?,
        end: postgres_int8bound("end", object.get("end"))?,
    })
}

fn postgres_int8bound(name: &str, payload: Option<&Value>) -> Result<postgres::Int8bound, String> {
    let (tag, payload) = tagged(needs_payload(name, payload)?)?;
    Ok(match tag {
        "included" => postgres::Int8bound::Included(int_payload(name, payload)?),
        "excluded" => postgres::Int8bound::Excluded(int_payload(name, payload)?),
        "unbounded" => postgres::Int8bound::Unbounded,
        other => return Err(bound_error(other)),
    })
}

fn postgres_numrange(tag: &str, payload: Option<&Value>) -> Result<postgres::Numrange, String> {
    let object = object_payload(tag, payload)?;
    Ok(postgres::Numrange {
        start: postgres_numbound("start", object.get("start"))?,
        end: postgres_numbound("end", object.get("end"))?,
    })
}

fn postgres_numbound(name: &str, payload: Option<&Value>) -> Result<postgres::Numbound, String> {
    let (tag, payload) = tagged(needs_payload(name, payload)?)?;
    Ok(match tag {
        "included" => postgres::Numbound::Included(text_payload(name, payload)?),
        "excluded" => postgres::Numbound::Excluded(text_payload(name, payload)?),
        "unbounded" => postgres::Numbound::Unbounded,
        other => return Err(bound_error(other)),
    })
}

fn postgres_tsrange(tag: &str, payload: Option<&Value>) -> Result<postgres::Tsrange, String> {
    let object = object_payload(tag, payload)?;
    Ok(postgres::Tsrange {
        start: postgres_tsbound("start", object.get("start"))?,
        end: postgres_tsbound("end", object.get("end"))?,
    })
}

fn postgres_tsbound(name: &str, payload: Option<&Value>) -> Result<postgres::Tsbound, String> {
    let (tag, payload) = tagged(needs_payload(name, payload)?)?;
    Ok(match tag {
        "included" => postgres::Tsbound::Included(timestamp_payload(name, payload)?),
        "excluded" => postgres::Tsbound::Excluded(timestamp_payload(name, payload)?),
        "unbounded" => postgres::Tsbound::Unbounded,
        other => return Err(bound_error(other)),
    })
}

fn postgres_tstzrange(tag: &str, payload: Option<&Value>) -> Result<postgres::Tstzrange, String> {
    let object = object_payload(tag, payload)?;
    Ok(postgres::Tstzrange {
        start: postgres_tstzbound("start", object.get("start"))?,
        end: postgres_tstzbound("end", object.get("end"))?,
    })
}

fn postgres_tstzbound(name: &str, payload: Option<&Value>) -> Result<postgres::Tstzbound, String> {
    let (tag, payload) = tagged(needs_payload(name, payload)?)?;
    Ok(match tag {
        "included" => postgres::Tstzbound::Included(timestamptz_payload(name, payload)?),
        "excluded" => postgres::Tstzbound::Excluded(timestamptz_payload(name, payload)?),
        "unbounded" => postgres::Tstzbound::Unbounded,
        other => return Err(bound_error(other)),
    })
}

fn postgres_daterange(tag: &str, payload: Option<&Value>) -> Result<postgres::Daterange, String> {
    let object = object_payload(tag, payload)?;
    Ok(postgres::Daterange {
        start: postgres_datebound("start", object.get("start"))?,
        end: postgres_datebound("end", object.get("end"))?,
    })
}

fn postgres_datebound(name: &str, payload: Option<&Value>) -> Result<postgres::Datebound, String> {
    let (tag, payload) = tagged(needs_payload(name, payload)?)?;
    Ok(match tag {
        "included" => postgres::Datebound::Included(date_payload(name, payload)?),
        "excluded" => postgres::Datebound::Excluded(date_payload(name, payload)?),
        "unbounded" => postgres::Datebound::Unbounded,
        other => return Err(bound_error(other)),
    })
}

fn bound_error(other: &str) -> String {
    format!("a range bound is included, excluded or unbounded, and \"{other}\" is none of them")
}

/// The postgres result as the JSON `dbQuery!` returns.
pub fn postgres_result(result: &postgres::DbResult) -> String {
    let columns: Vec<Value> = result
        .columns
        .iter()
        .map(|column| {
            json!({
                "ordinal": column.ordinal,
                "name": column.name,
                "dbType": postgres_column_type_json(&column.db_type),
                "dbTypeName": column.db_type_name,
            })
        })
        .collect();
    let rows: Vec<Value> = result
        .rows
        .iter()
        .map(|row| Value::Array(row.values.iter().map(postgres_value_json).collect()))
        .collect();
    json!({ "columns": columns, "rows": rows }).to_string()
}

fn postgres_column_type_json(column_type: &postgres::DbColumnType) -> Value {
    match column_type {
        postgres::DbColumnType::Enumeration(enumeration) => {
            json!({ "tag": "enumeration", "value": { "name": enumeration.name } })
        }
        postgres::DbColumnType::Composite(composite) => json!({
            "tag": "composite",
            "value": {
                "name": composite.name,
                "attributes": composite
                    .attributes
                    .iter()
                    .map(|(name, attribute)| json!([name, postgres_column_type_json(&attribute.get())]))
                    .collect::<Vec<_>>(),
            },
        }),
        postgres::DbColumnType::Domain(domain) => json!({
            "tag": "domain",
            "value": {
                "name": domain.name,
                "baseType": postgres_column_type_json(&domain.base_type.get()),
            },
        }),
        postgres::DbColumnType::Array(element) => json!({
            "tag": "array",
            "value": postgres_column_type_json(&element.get()),
        }),
        postgres::DbColumnType::Range(range) => json!({
            "tag": "range",
            "value": {
                "name": range.name,
                "baseType": postgres_column_type_json(&range.base_type.get()),
            },
        }),
        other => json!({ "tag": postgres_column_type_tag(other) }),
    }
}

fn postgres_column_type_tag(column_type: &postgres::DbColumnType) -> &'static str {
    match column_type {
        postgres::DbColumnType::Character => "character",
        postgres::DbColumnType::Int2 => "int2",
        postgres::DbColumnType::Int4 => "int4",
        postgres::DbColumnType::Int8 => "int8",
        postgres::DbColumnType::Float4 => "float4",
        postgres::DbColumnType::Float8 => "float8",
        postgres::DbColumnType::Numeric => "numeric",
        postgres::DbColumnType::Boolean => "boolean",
        postgres::DbColumnType::Text => "text",
        postgres::DbColumnType::Varchar => "varchar",
        postgres::DbColumnType::Bpchar => "bpchar",
        postgres::DbColumnType::Timestamp => "timestamp",
        postgres::DbColumnType::Timestamptz => "timestamptz",
        postgres::DbColumnType::Date => "date",
        postgres::DbColumnType::Time => "time",
        postgres::DbColumnType::Timetz => "timetz",
        postgres::DbColumnType::Interval => "interval",
        postgres::DbColumnType::Bytea => "bytea",
        postgres::DbColumnType::Uuid => "uuid",
        postgres::DbColumnType::Xml => "xml",
        postgres::DbColumnType::Json => "json",
        postgres::DbColumnType::Jsonb => "jsonb",
        postgres::DbColumnType::Jsonpath => "jsonpath",
        postgres::DbColumnType::Inet => "inet",
        postgres::DbColumnType::Cidr => "cidr",
        postgres::DbColumnType::Macaddr => "macaddr",
        postgres::DbColumnType::Bit => "bit",
        postgres::DbColumnType::Varbit => "varbit",
        postgres::DbColumnType::Int4range => "int4range",
        postgres::DbColumnType::Int8range => "int8range",
        postgres::DbColumnType::Numrange => "numrange",
        postgres::DbColumnType::Tsrange => "tsrange",
        postgres::DbColumnType::Tstzrange => "tstzrange",
        postgres::DbColumnType::Daterange => "daterange",
        postgres::DbColumnType::Money => "money",
        postgres::DbColumnType::Oid => "oid",
        postgres::DbColumnType::Vector => "vector",
        postgres::DbColumnType::Halfvec => "halfvec",
        postgres::DbColumnType::Sparsevec => "sparsevec",
        postgres::DbColumnType::Enumeration(_)
        | postgres::DbColumnType::Composite(_)
        | postgres::DbColumnType::Domain(_)
        | postgres::DbColumnType::Array(_)
        | postgres::DbColumnType::Range(_) => "(nested)",
    }
}

pub fn postgres_value_json(value: &postgres::DbValue) -> Value {
    match value {
        postgres::DbValue::Character(number) => json!({ "tag": "character", "value": number }),
        postgres::DbValue::Int2(number) => json!({ "tag": "int2", "value": number }),
        postgres::DbValue::Int4(number) => json!({ "tag": "int4", "value": number }),
        postgres::DbValue::Int8(number) => json!({ "tag": "int8", "value": number }),
        postgres::DbValue::Float4(number) => json!({ "tag": "float4", "value": float32_json(*number) }),
        postgres::DbValue::Float8(number) => json!({ "tag": "float8", "value": float_json(*number) }),
        postgres::DbValue::Numeric(number) => json!({ "tag": "numeric", "value": number }),
        postgres::DbValue::Boolean(flag) => json!({ "tag": "boolean", "value": flag }),
        postgres::DbValue::Text(text) => json!({ "tag": "text", "value": text }),
        postgres::DbValue::Varchar(text) => json!({ "tag": "varchar", "value": text }),
        postgres::DbValue::Bpchar(text) => json!({ "tag": "bpchar", "value": text }),
        postgres::DbValue::Timestamp(timestamp) => {
            json!({ "tag": "timestamp", "value": timestamp_json(timestamp) })
        }
        postgres::DbValue::Timestamptz(timestamp) => {
            json!({ "tag": "timestamptz", "value": timestamptz_json(timestamp) })
        }
        postgres::DbValue::Date(date) => json!({ "tag": "date", "value": date_json(date) }),
        postgres::DbValue::Time(time) => json!({ "tag": "time", "value": time_json(time) }),
        postgres::DbValue::Timetz(time) => json!({ "tag": "timetz", "value": timetz_json(time) }),
        postgres::DbValue::Interval(interval) => json!({
            "tag": "interval",
            "value": {
                "months": interval.months,
                "days": interval.days,
                "microseconds": interval.microseconds,
            },
        }),
        postgres::DbValue::Bytea(bytes) => json!({ "tag": "bytea", "value": bytes }),
        postgres::DbValue::Json(text) => json!({ "tag": "json", "value": text }),
        postgres::DbValue::Jsonb(text) => json!({ "tag": "jsonb", "value": text }),
        postgres::DbValue::Jsonpath(text) => json!({ "tag": "jsonpath", "value": text }),
        postgres::DbValue::Xml(text) => json!({ "tag": "xml", "value": text }),
        postgres::DbValue::Uuid(uuid) => json!({
            "tag": "uuid",
            "value": uuid_text(uuid.high_bits, uuid.low_bits),
        }),
        postgres::DbValue::Inet(address) => {
            json!({ "tag": "inet", "value": ip_address_json(address) })
        }
        postgres::DbValue::Cidr(address) => {
            json!({ "tag": "cidr", "value": ip_address_json(address) })
        }
        postgres::DbValue::Macaddr(address) => json!({
            "tag": "macaddr",
            "value": mac_address_json(address),
        }),
        postgres::DbValue::Bit(bits) => json!({ "tag": "bit", "value": bits }),
        postgres::DbValue::Varbit(bits) => json!({ "tag": "varbit", "value": bits }),
        postgres::DbValue::Int4range(range) => json!({
            "tag": "int4range",
            "value": { "start": postgres_int4bound_json(&range.start), "end": postgres_int4bound_json(&range.end) },
        }),
        postgres::DbValue::Int8range(range) => json!({
            "tag": "int8range",
            "value": { "start": postgres_int8bound_json(&range.start), "end": postgres_int8bound_json(&range.end) },
        }),
        postgres::DbValue::Numrange(range) => json!({
            "tag": "numrange",
            "value": { "start": postgres_numbound_json(&range.start), "end": postgres_numbound_json(&range.end) },
        }),
        postgres::DbValue::Tsrange(range) => json!({
            "tag": "tsrange",
            "value": { "start": postgres_tsbound_json(&range.start), "end": postgres_tsbound_json(&range.end) },
        }),
        postgres::DbValue::Tstzrange(range) => json!({
            "tag": "tstzrange",
            "value": { "start": postgres_tstzbound_json(&range.start), "end": postgres_tstzbound_json(&range.end) },
        }),
        postgres::DbValue::Daterange(range) => json!({
            "tag": "daterange",
            "value": { "start": postgres_datebound_json(&range.start), "end": postgres_datebound_json(&range.end) },
        }),
        postgres::DbValue::Money(amount) => json!({ "tag": "money", "value": amount }),
        postgres::DbValue::Oid(oid) => json!({ "tag": "oid", "value": oid }),
        postgres::DbValue::Enumeration(enumeration) => json!({
            "tag": "enumeration",
            "value": { "name": enumeration.name, "value": enumeration.value },
        }),
        postgres::DbValue::Composite(composite) => json!({
            "tag": "composite",
            "value": {
                "name": composite.name,
                "values": composite.values.iter().map(|value| postgres_value_json(&value.get())).collect::<Vec<_>>(),
            },
        }),
        postgres::DbValue::Domain(domain) => json!({
            "tag": "domain",
            "value": { "name": domain.name, "value": postgres_value_json(&domain.value.get()) },
        }),
        postgres::DbValue::Array(elements) => json!({
            "tag": "array",
            "value": elements.iter().map(|element| postgres_value_json(&element.get())).collect::<Vec<_>>(),
        }),
        postgres::DbValue::Range(range) => json!({
            "tag": "range",
            "value": {
                "name": range.name,
                "value": {
                    "start": postgres_value_bound_json(&range.value.start),
                    "end": postgres_value_bound_json(&range.value.end),
                },
            },
        }),
        postgres::DbValue::Null => json!({ "tag": "null" }),
        postgres::DbValue::Vector(numbers) => json!({ "tag": "vector", "value": numbers }),
        postgres::DbValue::Halfvec(numbers) => json!({ "tag": "halfvec", "value": numbers }),
        postgres::DbValue::Sparsevec(sparse) => json!({
            "tag": "sparsevec",
            "value": { "dim": sparse.dim, "indices": sparse.indices, "values": sparse.values },
        }),
    }
}

fn postgres_int4bound_json(bound: &postgres::Int4bound) -> Value {
    match bound {
        postgres::Int4bound::Included(number) => json!({ "tag": "included", "value": number }),
        postgres::Int4bound::Excluded(number) => json!({ "tag": "excluded", "value": number }),
        postgres::Int4bound::Unbounded => json!({ "tag": "unbounded" }),
    }
}

fn postgres_int8bound_json(bound: &postgres::Int8bound) -> Value {
    match bound {
        postgres::Int8bound::Included(number) => json!({ "tag": "included", "value": number }),
        postgres::Int8bound::Excluded(number) => json!({ "tag": "excluded", "value": number }),
        postgres::Int8bound::Unbounded => json!({ "tag": "unbounded" }),
    }
}

fn postgres_numbound_json(bound: &postgres::Numbound) -> Value {
    match bound {
        postgres::Numbound::Included(number) => json!({ "tag": "included", "value": number }),
        postgres::Numbound::Excluded(number) => json!({ "tag": "excluded", "value": number }),
        postgres::Numbound::Unbounded => json!({ "tag": "unbounded" }),
    }
}

fn postgres_tsbound_json(bound: &postgres::Tsbound) -> Value {
    match bound {
        postgres::Tsbound::Included(timestamp) => {
            json!({ "tag": "included", "value": timestamp_json(timestamp) })
        }
        postgres::Tsbound::Excluded(timestamp) => {
            json!({ "tag": "excluded", "value": timestamp_json(timestamp) })
        }
        postgres::Tsbound::Unbounded => json!({ "tag": "unbounded" }),
    }
}

fn postgres_tstzbound_json(bound: &postgres::Tstzbound) -> Value {
    match bound {
        postgres::Tstzbound::Included(timestamp) => {
            json!({ "tag": "included", "value": timestamptz_json(timestamp) })
        }
        postgres::Tstzbound::Excluded(timestamp) => {
            json!({ "tag": "excluded", "value": timestamptz_json(timestamp) })
        }
        postgres::Tstzbound::Unbounded => json!({ "tag": "unbounded" }),
    }
}

fn postgres_datebound_json(bound: &postgres::Datebound) -> Value {
    match bound {
        postgres::Datebound::Included(date) => json!({ "tag": "included", "value": date_json(date) }),
        postgres::Datebound::Excluded(date) => json!({ "tag": "excluded", "value": date_json(date) }),
        postgres::Datebound::Unbounded => json!({ "tag": "unbounded" }),
    }
}

fn postgres_value_bound_json(bound: &postgres::ValueBound) -> Value {
    match bound {
        postgres::ValueBound::Included(value) => {
            json!({ "tag": "included", "value": postgres_value_json(&value.get()) })
        }
        postgres::ValueBound::Excluded(value) => {
            json!({ "tag": "excluded", "value": postgres_value_json(&value.get()) })
        }
        postgres::ValueBound::Unbounded => json!({ "tag": "unbounded" }),
    }
}

// --------------------------------------------------------------------- MySQL

/// The MySQL params: each element decoded into `golem:rdbms/mysql`'s
/// `db-value`.
pub fn mysql_params(items: &[Value]) -> Result<Vec<mysql::DbValue>, String> {
    items.iter().map(mysql_value).collect()
}

fn mysql_value(value: &Value) -> Result<mysql::DbValue, String> {
    let (tag, payload) = tagged(value)?;
    Ok(match tag {
        "boolean" => mysql::DbValue::Boolean(bool_payload(tag, payload)?),
        "tinyint" => mysql::DbValue::Tinyint(i8_payload(tag, payload)?),
        "smallint" => mysql::DbValue::Smallint(i16_payload(tag, payload)?),
        "mediumint" => mysql::DbValue::Mediumint(i32_payload(tag, payload)?),
        "int" => mysql::DbValue::Int(i32_payload(tag, payload)?),
        "bigint" => mysql::DbValue::Bigint(int_payload(tag, payload)?),
        "tinyintUnsigned" => mysql::DbValue::TinyintUnsigned(
            u8::try_from(uint_payload(tag, payload)?)
                .map_err(|_| String::from("the db-value \"tinyintUnsigned\" needs a byte"))?,
        ),
        "smallintUnsigned" => mysql::DbValue::SmallintUnsigned(u16_payload(tag, payload)?),
        "mediumintUnsigned" => mysql::DbValue::MediumintUnsigned(u32_payload(tag, payload)?),
        "intUnsigned" => mysql::DbValue::IntUnsigned(u32_payload(tag, payload)?),
        "bigintUnsigned" => mysql::DbValue::BigintUnsigned(uint_payload(tag, payload)?),
        "float" => mysql::DbValue::Float(float_payload(tag, payload)? as f32),
        "double" => mysql::DbValue::Double(float_payload(tag, payload)?),
        "decimal" => mysql::DbValue::Decimal(text_payload(tag, payload)?),
        "date" => mysql::DbValue::Date(date_payload(tag, payload)?),
        "datetime" => mysql::DbValue::Datetime(timestamp_payload(tag, payload)?),
        "timestamp" => mysql::DbValue::Timestamp(timestamp_payload(tag, payload)?),
        "time" => mysql::DbValue::Time(time_payload(tag, payload)?),
        "year" => mysql::DbValue::Year(u16_payload(tag, payload)?),
        "fixchar" => mysql::DbValue::Fixchar(text_payload(tag, payload)?),
        "varchar" => mysql::DbValue::Varchar(text_payload(tag, payload)?),
        "tinytext" => mysql::DbValue::Tinytext(text_payload(tag, payload)?),
        "text" => mysql::DbValue::Text(text_payload(tag, payload)?),
        "mediumtext" => mysql::DbValue::Mediumtext(text_payload(tag, payload)?),
        "longtext" => mysql::DbValue::Longtext(text_payload(tag, payload)?),
        "binary" => mysql::DbValue::Binary(bytes_payload(tag, payload)?),
        "varbinary" => mysql::DbValue::Varbinary(bytes_payload(tag, payload)?),
        "tinyblob" => mysql::DbValue::Tinyblob(bytes_payload(tag, payload)?),
        "blob" => mysql::DbValue::Blob(bytes_payload(tag, payload)?),
        "mediumblob" => mysql::DbValue::Mediumblob(bytes_payload(tag, payload)?),
        "longblob" => mysql::DbValue::Longblob(bytes_payload(tag, payload)?),
        "enumeration" => mysql::DbValue::Enumeration(text_payload(tag, payload)?),
        "set" => mysql::DbValue::Set(text_payload(tag, payload)?),
        "bit" => mysql::DbValue::Bit(bits_payload(tag, payload)?),
        "json" => mysql::DbValue::Json(text_payload(tag, payload)?),
        "null" => {
            needs_no_payload(tag, payload)?;
            mysql::DbValue::Null
        }
        other => {
            return Err(format!(
                "unknown db-value tag \"{other}\" for a mysql connection"
            ))
        }
    })
}

/// The MySQL result as the JSON `dbQuery!` returns.
pub fn mysql_result(result: &mysql::DbResult) -> String {
    let columns: Vec<Value> = result
        .columns
        .iter()
        .map(|column| {
            json!({
                "ordinal": column.ordinal,
                "name": column.name,
                "dbType": mysql_column_type_json(&column.db_type),
                "dbTypeName": column.db_type_name,
            })
        })
        .collect();
    let rows: Vec<Value> = result
        .rows
        .iter()
        .map(|row| Value::Array(row.values.iter().map(mysql_value_json).collect()))
        .collect();
    json!({ "columns": columns, "rows": rows }).to_string()
}

fn mysql_column_type_json(column_type: &mysql::DbColumnType) -> Value {
    json!({ "tag": mysql_column_type_tag(column_type) })
}

fn mysql_column_type_tag(column_type: &mysql::DbColumnType) -> &'static str {
    match column_type {
        mysql::DbColumnType::Boolean => "boolean",
        mysql::DbColumnType::Tinyint => "tinyint",
        mysql::DbColumnType::Smallint => "smallint",
        mysql::DbColumnType::Mediumint => "mediumint",
        mysql::DbColumnType::Int => "int",
        mysql::DbColumnType::Bigint => "bigint",
        mysql::DbColumnType::TinyintUnsigned => "tinyintUnsigned",
        mysql::DbColumnType::SmallintUnsigned => "smallintUnsigned",
        mysql::DbColumnType::MediumintUnsigned => "mediumintUnsigned",
        mysql::DbColumnType::IntUnsigned => "intUnsigned",
        mysql::DbColumnType::BigintUnsigned => "bigintUnsigned",
        mysql::DbColumnType::Float => "float",
        mysql::DbColumnType::Double => "double",
        mysql::DbColumnType::Decimal => "decimal",
        mysql::DbColumnType::Date => "date",
        mysql::DbColumnType::Datetime => "datetime",
        mysql::DbColumnType::Timestamp => "timestamp",
        mysql::DbColumnType::Time => "time",
        mysql::DbColumnType::Year => "year",
        mysql::DbColumnType::Fixchar => "fixchar",
        mysql::DbColumnType::Varchar => "varchar",
        mysql::DbColumnType::Tinytext => "tinytext",
        mysql::DbColumnType::Text => "text",
        mysql::DbColumnType::Mediumtext => "mediumtext",
        mysql::DbColumnType::Longtext => "longtext",
        mysql::DbColumnType::Binary => "binary",
        mysql::DbColumnType::Varbinary => "varbinary",
        mysql::DbColumnType::Tinyblob => "tinyblob",
        mysql::DbColumnType::Blob => "blob",
        mysql::DbColumnType::Mediumblob => "mediumblob",
        mysql::DbColumnType::Longblob => "longblob",
        mysql::DbColumnType::Enumeration => "enumeration",
        mysql::DbColumnType::Set => "set",
        mysql::DbColumnType::Bit => "bit",
        mysql::DbColumnType::Json => "json",
    }
}

pub fn mysql_value_json(value: &mysql::DbValue) -> Value {
    match value {
        mysql::DbValue::Boolean(flag) => json!({ "tag": "boolean", "value": flag }),
        mysql::DbValue::Tinyint(number) => json!({ "tag": "tinyint", "value": number }),
        mysql::DbValue::Smallint(number) => json!({ "tag": "smallint", "value": number }),
        mysql::DbValue::Mediumint(number) => json!({ "tag": "mediumint", "value": number }),
        mysql::DbValue::Int(number) => json!({ "tag": "int", "value": number }),
        mysql::DbValue::Bigint(number) => json!({ "tag": "bigint", "value": number }),
        mysql::DbValue::TinyintUnsigned(number) => json!({ "tag": "tinyintUnsigned", "value": number }),
        mysql::DbValue::SmallintUnsigned(number) => json!({ "tag": "smallintUnsigned", "value": number }),
        mysql::DbValue::MediumintUnsigned(number) => json!({ "tag": "mediumintUnsigned", "value": number }),
        mysql::DbValue::IntUnsigned(number) => json!({ "tag": "intUnsigned", "value": number }),
        mysql::DbValue::BigintUnsigned(number) => json!({ "tag": "bigintUnsigned", "value": number }),
        mysql::DbValue::Float(number) => json!({ "tag": "float", "value": float32_json(*number) }),
        mysql::DbValue::Double(number) => json!({ "tag": "double", "value": float_json(*number) }),
        mysql::DbValue::Decimal(number) => json!({ "tag": "decimal", "value": number }),
        mysql::DbValue::Date(date) => json!({ "tag": "date", "value": date_json(date) }),
        mysql::DbValue::Datetime(timestamp) => json!({ "tag": "datetime", "value": timestamp_json(timestamp) }),
        mysql::DbValue::Timestamp(timestamp) => json!({ "tag": "timestamp", "value": timestamp_json(timestamp) }),
        mysql::DbValue::Time(time) => json!({ "tag": "time", "value": time_json(time) }),
        mysql::DbValue::Year(year) => json!({ "tag": "year", "value": year }),
        mysql::DbValue::Fixchar(text) => json!({ "tag": "fixchar", "value": text }),
        mysql::DbValue::Varchar(text) => json!({ "tag": "varchar", "value": text }),
        mysql::DbValue::Tinytext(text) => json!({ "tag": "tinytext", "value": text }),
        mysql::DbValue::Text(text) => json!({ "tag": "text", "value": text }),
        mysql::DbValue::Mediumtext(text) => json!({ "tag": "mediumtext", "value": text }),
        mysql::DbValue::Longtext(text) => json!({ "tag": "longtext", "value": text }),
        mysql::DbValue::Binary(bytes) => json!({ "tag": "binary", "value": bytes }),
        mysql::DbValue::Varbinary(bytes) => json!({ "tag": "varbinary", "value": bytes }),
        mysql::DbValue::Tinyblob(bytes) => json!({ "tag": "tinyblob", "value": bytes }),
        mysql::DbValue::Blob(bytes) => json!({ "tag": "blob", "value": bytes }),
        mysql::DbValue::Mediumblob(bytes) => json!({ "tag": "mediumblob", "value": bytes }),
        mysql::DbValue::Longblob(bytes) => json!({ "tag": "longblob", "value": bytes }),
        mysql::DbValue::Enumeration(text) => json!({ "tag": "enumeration", "value": text }),
        mysql::DbValue::Set(text) => json!({ "tag": "set", "value": text }),
        mysql::DbValue::Bit(bits) => json!({ "tag": "bit", "value": bits }),
        mysql::DbValue::Json(text) => json!({ "tag": "json", "value": text }),
        mysql::DbValue::Null => json!({ "tag": "null" }),
    }
}

// -------------------------------------------------------------------- Ignite

/// The Ignite params: each element decoded into `golem:rdbms/ignite2`'s
/// `db-value`.
pub fn ignite_params(items: &[Value]) -> Result<Vec<ignite2::DbValue>, String> {
    items.iter().map(ignite_value).collect()
}

fn ignite_value(value: &Value) -> Result<ignite2::DbValue, String> {
    let (tag, payload) = tagged(value)?;
    Ok(match tag {
        "dbBoolean" => ignite2::DbValue::DbBoolean(bool_payload(tag, payload)?),
        "dbByte" => ignite2::DbValue::DbByte(i8_payload(tag, payload)?),
        "dbShort" => ignite2::DbValue::DbShort(i16_payload(tag, payload)?),
        "dbInt" => ignite2::DbValue::DbInt(i32_payload(tag, payload)?),
        "dbLong" => ignite2::DbValue::DbLong(int_payload(tag, payload)?),
        "dbFloat" => ignite2::DbValue::DbFloat(float_payload(tag, payload)? as f32),
        "dbDouble" => ignite2::DbValue::DbDouble(float_payload(tag, payload)?),
        "dbChar" => ignite2::DbValue::DbChar(u16_payload(tag, payload)?),
        "dbString" => ignite2::DbValue::DbString(text_payload(tag, payload)?),
        "dbUuid" => {
            let (high, low) = uuid_bits(&text_payload(tag, payload)?)?;
            ignite2::DbValue::DbUuid((high, low))
        }
        "dbDate" => ignite2::DbValue::DbDate(int_payload(tag, payload)?),
        "dbTimestamp" => {
            let pair = array_payload(tag, payload)?;
            if pair.len() != 2 {
                return Err(String::from(
                    "the db-value \"dbTimestamp\" needs [milliseconds, nanoseconds]",
                ));
            }
            ignite2::DbValue::DbTimestamp((
                int_payload(tag, pair.first())?,
                i32_payload(tag, pair.get(1))?,
            ))
        }
        "dbTime" => ignite2::DbValue::DbTime(int_payload(tag, payload)?),
        "dbDecimal" => ignite2::DbValue::DbDecimal(text_payload(tag, payload)?),
        "dbByteArray" => ignite2::DbValue::DbByteArray(bytes_payload(tag, payload)?),
        "dbNull" => {
            needs_no_payload(tag, payload)?;
            ignite2::DbValue::DbNull
        }
        other => {
            return Err(format!(
                "unknown db-value tag \"{other}\" for an ignite connection"
            ))
        }
    })
}

/// The Ignite result as the JSON `dbQuery!` returns. Its `db-column` carries no
/// type — the interface has no column-type vocabulary — so a column is its
/// ordinal and name only.
pub fn ignite_result(result: &ignite2::DbResult) -> String {
    let columns: Vec<Value> = result
        .columns
        .iter()
        .map(|column| json!({ "ordinal": column.ordinal, "name": column.name }))
        .collect();
    let rows: Vec<Value> = result
        .rows
        .iter()
        .map(|row| Value::Array(row.values.iter().map(ignite_value_json).collect()))
        .collect();
    json!({ "columns": columns, "rows": rows }).to_string()
}

pub fn ignite_value_json(value: &ignite2::DbValue) -> Value {
    match value {
        ignite2::DbValue::DbNull => json!({ "tag": "dbNull" }),
        ignite2::DbValue::DbBoolean(flag) => json!({ "tag": "dbBoolean", "value": flag }),
        ignite2::DbValue::DbByte(number) => json!({ "tag": "dbByte", "value": number }),
        ignite2::DbValue::DbShort(number) => json!({ "tag": "dbShort", "value": number }),
        ignite2::DbValue::DbInt(number) => json!({ "tag": "dbInt", "value": number }),
        ignite2::DbValue::DbLong(number) => json!({ "tag": "dbLong", "value": number }),
        ignite2::DbValue::DbFloat(number) => json!({ "tag": "dbFloat", "value": float32_json(*number) }),
        ignite2::DbValue::DbDouble(number) => json!({ "tag": "dbDouble", "value": float_json(*number) }),
        ignite2::DbValue::DbChar(character) => json!({ "tag": "dbChar", "value": character }),
        ignite2::DbValue::DbString(text) => json!({ "tag": "dbString", "value": text }),
        ignite2::DbValue::DbUuid((high, low)) => json!({
            "tag": "dbUuid",
            "value": uuid_text(*high, *low),
        }),
        ignite2::DbValue::DbDate(millis) => json!({ "tag": "dbDate", "value": millis }),
        ignite2::DbValue::DbTimestamp((millis, nanos)) => json!({
            "tag": "dbTimestamp",
            "value": [millis, nanos],
        }),
        ignite2::DbValue::DbTime(nanos) => json!({ "tag": "dbTime", "value": nanos }),
        ignite2::DbValue::DbDecimal(number) => json!({ "tag": "dbDecimal", "value": number }),
        ignite2::DbValue::DbByteArray(bytes) => json!({ "tag": "dbByteArray", "value": bytes }),
    }
}
