import Types exposing [Result]

Parse := [].{

	## Parse an unsigned 64-bit integer from a string with overflow checking.
	## Automatically trims leading and trailing whitespace.
	parseU64 : Str -> Result(U64, [InvalidNumStr, Overflow])
	parseU64 = |raw| {
		trimmed = Str.trim(raw)
		bytes = Str.to_utf8(trimmed)
		match bytes {
			[43, .. as rest] =>
				if List.is_empty(rest) {
					Err(InvalidNumStr)
				} else {
					parseU64Bytes(rest)
				}
			_ => parseU64Bytes(bytes)
		}
	}

	## Parse unsigned 64-bit integer, returning a fallback default value on error.
	parseU64Or : Str, U64 -> U64
	parseU64Or = |raw, defaultVal|
		match parseU64(raw) {
			Ok(v) => v
			Err(_) => defaultVal
		}

	## Parse an unsigned 32-bit integer with overflow checking.
	parseU32 : Str -> Result(U32, [InvalidNumStr, Overflow])
	parseU32 = |raw|
		match parseU64(raw) {
			Ok(v) =>
				match U64.to_u32_try(v) {
					Ok(u) => Ok(u)
					Err(_) => Err(Overflow)
				}
			Err(e) => Err(e)
		}

	## Parse unsigned 32-bit integer, returning a fallback default value on error.
	parseU32Or : Str, U32 -> U32
	parseU32Or = |raw, defaultVal|
		match parseU32(raw) {
			Ok(v) => v
			Err(_) => defaultVal
		}

	## Parse an unsigned 16-bit integer with overflow checking.
	parseU16 : Str -> Result(U16, [InvalidNumStr, Overflow])
	parseU16 = |raw|
		match parseU64(raw) {
			Ok(v) =>
				match U64.to_u16_try(v) {
					Ok(u) => Ok(u)
					Err(_) => Err(Overflow)
				}
			Err(e) => Err(e)
		}

	## Parse an unsigned 8-bit integer with overflow checking.
	parseU8 : Str -> Result(U8, [InvalidNumStr, Overflow])
	parseU8 = |raw|
		match parseU64(raw) {
			Ok(v) =>
				match U64.to_u8_try(v) {
					Ok(u) => Ok(u)
					Err(_) => Err(Overflow)
				}
			Err(e) => Err(e)
		}

	## Parse a signed 64-bit integer with sign handling ('+', '-') and overflow checking.
	## Automatically trims leading and trailing whitespace.
	parseI64 : Str -> Result(I64, [InvalidNumStr, Overflow])
	parseI64 = |raw| {
		trimmed = Str.trim(raw)
		bytes = Str.to_utf8(trimmed)
		match bytes {
			[] => Err(InvalidNumStr)
			[45, .. as rest] =>
			# Negative sign '-'
				if List.is_empty(rest) {
					Err(InvalidNumStr)
				} else {
					match parseU64Bytes(rest) {
						Ok(val) =>
							if val > 9223372036854775808.U64 {
								Err(Overflow)
							} else if val == 9223372036854775808.U64 {
								Ok(-9223372036854775808.I64)
							} else {
								match U64.to_i64_try(val) {
									Ok(signed) => Ok(-signed)
									Err(_) => Err(Overflow)
								}
							}
						Err(e) => Err(e)
					}
				}
			[43, .. as rest] =>
			# Positive sign '+'
				if List.is_empty(rest) {
					Err(InvalidNumStr)
				} else {
					match parseU64Bytes(rest) {
						Ok(val) =>
							match U64.to_i64_try(val) {
								Ok(signed) => Ok(signed)
								Err(_) => Err(Overflow)
							}
						Err(e) => Err(e)
					}
				}
			_ =>
				match parseU64Bytes(bytes) {
					Ok(val) =>
						match U64.to_i64_try(val) {
							Ok(signed) => Ok(signed)
							Err(_) => Err(Overflow)
						}
					Err(e) => Err(e)
				}
		}
	}

	## Parse signed 64-bit integer, returning a fallback default value on error.
	parseI64Or : Str, I64 -> I64
	parseI64Or = |raw, defaultVal|
		match parseI64(raw) {
			Ok(v) => v
			Err(_) => defaultVal
		}

	## Parse a signed 32-bit integer with sign handling and overflow checking.
	parseI32 : Str -> Result(I32, [InvalidNumStr, Overflow])
	parseI32 = |raw|
		match parseI64(raw) {
			Ok(v) =>
				match I64.to_i32_try(v) {
					Ok(i) => Ok(i)
					Err(_) => Err(Overflow)
				}
			Err(e) => Err(e)
		}

	## Parse signed 32-bit integer, returning a fallback default value on error.
	parseI32Or : Str, I32 -> I32
	parseI32Or = |raw, defaultVal|
		match parseI32(raw) {
			Ok(v) => v
			Err(_) => defaultVal
		}

	## Parse a signed 16-bit integer with sign handling and overflow checking.
	parseI16 : Str -> Result(I16, [InvalidNumStr, Overflow])
	parseI16 = |raw|
		match parseI64(raw) {
			Ok(v) =>
				match I64.to_i16_try(v) {
					Ok(i) => Ok(i)
					Err(_) => Err(Overflow)
				}
			Err(e) => Err(e)
		}

	## Parse a signed 8-bit integer with sign handling and overflow checking.
	parseI8 : Str -> Result(I8, [InvalidNumStr, Overflow])
	parseI8 = |raw|
		match parseI64(raw) {
			Ok(v) =>
				match I64.to_i8_try(v) {
					Ok(i) => Ok(i)
					Err(_) => Err(Overflow)
				}
			Err(e) => Err(e)
		}

	## Parse 64-bit IEEE-754 floating point number from string.
	parseF64 : Str -> Result(F64, [InvalidNumStr])
	parseF64 = |raw|
		match F64.from_str(Str.trim(raw)) {
			Ok(val) => Ok(val)
			Err(_) => Err(InvalidNumStr)
		}

	## Parse float or return fallback default value
	parseF64Or : Str, F64 -> F64
	parseF64Or = |raw, defaultVal|
		match parseF64(raw) {
			Ok(v) => v
			Err(_) => defaultVal
		}
}

## Internal helper: parse U64 from raw ASCII bytes
parseU64Bytes : List(U8) -> Result(U64, [InvalidNumStr, Overflow])
parseU64Bytes = |bytes| {
	if List.is_empty(bytes) {
		Err(InvalidNumStr)
	} else {
		cutoff = 1844674407370955161.U64
		maxLastDigit = 5.U64
		List.fold_try(
			bytes,
			0.U64,
			|acc, b|
				if b >= 48 and b <= 57 {
					digit = U8.to_u64(b - 48)
					if acc > cutoff or (acc == cutoff and digit > maxLastDigit) {
						Err(Overflow)
					} else {
						Ok(acc * 10 + digit)
					}
				} else {
					Err(InvalidNumStr)
				},
		)
	}
}
