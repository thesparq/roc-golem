app [main] { pf: platform "../platform/main.roc" }

import pf.Parse exposing [
	parseF64,
	parseF64Or,
	parseI16,
	parseI32,
	parseI32Or,
	parseI64,
	parseI64Or,
	parseI8,
	parseU16,
	parseU32,
	parseU32Or,
	parseU64,
	parseU64Or,
	parseU8,
]
import pf.Golem exposing [Agent, build, define]
import pf.GolemJson

State : { count : U64 }

# --- U64 Tests ---

# Happy path
expect parseU64("42") == Ok(42.U64)
expect parseU64("0") == Ok(0.U64)
expect parseU64("18446744073709551615") == Ok(18446744073709551615.U64)
expect parseU64("+100") == Ok(100.U64)
expect parseU64("  12345  \n\t") == Ok(12345.U64)

# Error cases
expect parseU64("") == Err(InvalidNumStr)
expect parseU64("   ") == Err(InvalidNumStr)
expect parseU64("-1") == Err(InvalidNumStr)
expect parseU64("+") == Err(InvalidNumStr)
expect parseU64("123a45") == Err(InvalidNumStr)
expect parseU64("18446744073709551616") == Err(Overflow)
expect parseU64("99999999999999999999999") == Err(Overflow)

# --- U32, U16, U8 Tests ---

expect parseU32("4294967295") == Ok(4294967295.U32)
expect parseU32("4294967296") == Err(Overflow)
expect parseU32("100") == Ok(100.U32)

expect parseU16("65535") == Ok(65535.U16)
expect parseU16("65536") == Err(Overflow)
expect parseU16("50") == Ok(50.U16)

expect parseU8("255") == Ok(255.U8)
expect parseU8("256") == Err(Overflow)
expect parseU8("12") == Ok(12.U8)

# --- I64 Tests ---

expect parseI64("0") == Ok(0.I64)
expect parseI64("+0") == Ok(0.I64)
expect parseI64("-0") == Ok(0.I64)
expect parseI64("42") == Ok(42.I64)
expect parseI64("+42") == Ok(42.I64)
expect parseI64("-42") == Ok(-42.I64)
expect parseI64("  -999  ") == Ok(-999.I64)
expect parseI64("9223372036854775807") == Ok(9223372036854775807.I64)
expect parseI64("-9223372036854775808") == Ok(-9223372036854775808.I64)

# I64 Overflows & Errors
expect parseI64("9223372036854775808") == Err(Overflow)
expect parseI64("-9223372036854775809") == Err(Overflow)
expect parseI64("-") == Err(InvalidNumStr)
expect parseI64("+") == Err(InvalidNumStr)
expect parseI64("abc") == Err(InvalidNumStr)

# --- I32, I16, I8 Tests ---

expect parseI32("2147483647") == Ok(2147483647.I32)
expect parseI32("-2147483648") == Ok(-2147483648.I32)
expect parseI32("2147483648") == Err(Overflow)
expect parseI32("-2147483649") == Err(Overflow)

expect parseI16("32767") == Ok(32767.I16)
expect parseI16("-32768") == Ok(-32768.I16)
expect parseI16("32768") == Err(Overflow)
expect parseI16("-32769") == Err(Overflow)

expect parseI8("127") == Ok(127.I8)
expect parseI8("-128") == Ok(-128.I8)
expect parseI8("128") == Err(Overflow)
expect parseI8("-129") == Err(Overflow)

# --- F64 Tests ---

expect parseF64("0.0") == Ok(0.0.F64)
expect parseF64("-0.5") == Ok(-0.5.F64)
expect parseF64("123.456") == Ok(123.456.F64)
expect parseF64("100") == Ok(100.0.F64)
expect parseF64("not_a_number") == Err(InvalidNumStr)

# --- Fallback Or Helpers ---

expect parseU64Or("42", 10.U64) == 42.U64
expect parseU64Or("bad", 10.U64) == 10.U64
expect parseU32Or("100", 5.U32) == 100.U32
expect parseU32Or("99999999999", 5.U32) == 5.U32

expect parseI64Or("-500", 0.I64) == -500.I64
expect parseI64Or("bad", 99.I64) == 99.I64
expect parseI32Or("-20", 0.I32) == -20.I32
expect parseI32Or("invalid", 1.I32) == 1.I32

expect parseF64Or("3.14", 0.0.F64) == 3.14.F64
expect parseF64Or("bad", 2.718.F64) == 2.718.F64

# Dummy agent to satisfy platform requirements
agent : Agent(State)
agent =
	define(
		{ name: "test-parse-numbers", version: "0.1.0", description: "Test agent for numeric parsing" },
		|_| Ok({ count: 0.U64 }),
		|st| GolemJson.encode(st),
		|raw| GolemJson.decode(raw),
	)
		|> build

main = agent
