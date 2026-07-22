#![no_std]
#![allow(static_mut_refs)]

use core::arch::wasm32;
use core::sync::atomic::{AtomicUsize, Ordering};

const HEAP_START: usize = 0x10000;
static ALLOC_OFFSET: AtomicUsize = AtomicUsize::new(HEAP_START);

const STATE_BUF_SIZE: usize = 65536;
static mut STATE_BUF: [u8; STATE_BUF_SIZE] = [0; STATE_BUF_SIZE];
static mut STATE_LEN: usize = 0;

fn ensure_memory(min_size: usize) {
    let pages_needed = (min_size + 0xFFFF) / 0x10000;
    let current_pages = wasm32::memory_size(0);
    if pages_needed > current_pages {
        wasm32::memory_grow(0, pages_needed - current_pages);
    }
}

fn alloc(size: usize, align: usize) -> i32 {
    let ptr = cabi_realloc(core::ptr::null_mut(), 0, align, size) as i32;
    ensure_memory(ptr as usize + size);
    ptr
}

#[export_name = "cabi_realloc"]
pub extern "C" fn cabi_realloc(
    _old_ptr: *mut u8,
    _old_size: usize,
    align: usize,
    new_size: usize,
) -> *mut u8 {
    let offset = ALLOC_OFFSET.fetch_add(new_size, Ordering::SeqCst);
    let aligned = (offset + align - 1) & !(align - 1);
    ALLOC_OFFSET.store(aligned + new_size, Ordering::SeqCst);
    aligned as *mut u8
}

extern "C" {
    fn roc_golem_initialize(agent_type_ptr: i32, input_ptr: i32, state_output_ptr: i32);
    fn roc_golem_invoke(method_ptr: i32, state_ptr: i32, input_ptr: i32, output_ptr: i32);
    fn roc_golem_get_definition(output_ptr: i32);
    fn roc_golem_discover_types(output_ptr: i32);
    fn roc_golem_save(state_ptr: i32, output_ptr: i32);
    fn roc_golem_load(snapshot_ptr: i32, state_output_ptr: i32);
}

// --- Canon ABI encoding helpers ---

unsafe fn write_u8(buf: *mut u8, off: i32, val: u8) -> i32 {
    core::ptr::write_unaligned(buf.offset(off as isize) as *mut u8, val);
    off + 1
}

unsafe fn write_i32(buf: *mut u8, off: i32, val: i32) -> i32 {
    core::ptr::write_unaligned(buf.offset(off as isize) as *mut i32, val);
    off + 4
}

unsafe fn write_str(buf: *mut u8, off: i32, val: &str) -> i32 {
    let len = val.len() as i32;
    let off = write_i32(buf, off, len);
    if len > 0 {
        core::ptr::copy_nonoverlapping(val.as_ptr(), buf.offset(off as isize), val.len());
    }
    off + len
}

unsafe fn write_empty_list(buf: *mut u8, off: i32) -> i32 {
    write_i32(buf, off, 0)
}

unsafe fn write_variant_disc(buf: *mut u8, off: i32, disc: u8) -> i32 {
    write_u8(buf, off, disc)
}

unsafe fn write_option_none(buf: *mut u8, off: i32) -> i32 {
    write_u8(buf, off, 0)
}

unsafe fn write_enum(buf: *mut u8, off: i32, disc: u8) -> i32 {
    write_u8(buf, off, disc)
}

// --- Result encodings ---

unsafe fn encode_result_ok_empty(buf: *mut u8, off: i32) -> i32 {
    write_variant_disc(buf, off, 0)
}

unsafe fn encode_result_err_generic(buf: *mut u8, off: i32) -> i32 {
    let off = write_variant_disc(buf, off, 1);
    let off = write_variant_disc(buf, off, 0);
    write_str(buf, off, "roc error")
}

unsafe fn encode_result_ok_data_value_empty(buf: *mut u8, off: i32) -> i32 {
    let off = write_variant_disc(buf, off, 0);
    let off = write_variant_disc(buf, off, 0);
    write_empty_list(buf, off)
}

unsafe fn encode_result_ok_empty_agent_type_list(buf: *mut u8, off: i32) -> i32 {
    let off = write_variant_disc(buf, off, 0);
    write_empty_list(buf, off)
}

unsafe fn encode_result_ok_unit(buf: *mut u8, off: i32) -> i32 {
    write_variant_disc(buf, off, 0)
}

/// snapshot { payload: list<u8>, mime-type: string }
unsafe fn encode_snapshot(buf: *mut u8, off: i32, payload: &[u8], mime: &str) -> i32 {
    let off = write_i32(buf, off, payload.len() as i32);
    if !payload.is_empty() {
        core::ptr::copy_nonoverlapping(payload.as_ptr(), buf.offset(off as isize), payload.len());
    }
    let off = off + payload.len() as i32;
    write_str(buf, off, mime)
}

unsafe fn encode_minimal_agent_type(buf: *mut u8, off: i32, type_name: &str) -> i32 {
    let mut o = off;
    o = write_str(buf, o, type_name);
    o = write_str(buf, o, "");
    o = write_str(buf, o, "roc");
    o = write_option_none(buf, o);
    o = write_str(buf, o, "");
    o = write_option_none(buf, o);
    o = write_variant_disc(buf, o, 0);
    o = write_empty_list(buf, o);
    o = write_empty_list(buf, o);
    o = write_empty_list(buf, o);
    o = write_enum(buf, o, 1);
    o = write_option_none(buf, o);
    o = write_variant_disc(buf, o, 1);
    o = write_variant_disc(buf, o, 0);
    o = write_empty_list(buf, o);
    o
}

// --- Roc string helpers ---

fn alloc_roc_string(data: &[u8]) -> i32 {
    let len = data.len();
    let ptr = alloc(len + 4, 4);
    unsafe {
        core::ptr::write_unaligned(ptr as *mut i32, len as i32);
        if len > 0 {
            core::ptr::copy_nonoverlapping(data.as_ptr(), (ptr + 4) as *mut u8, len);
        }
    }
    ptr
}

unsafe fn canon_string_to_roc(buf: *const u8, offset: i32) -> (i32, i32) {
    let len = core::ptr::read_unaligned(buf.offset(offset as isize) as *const i32);
    let roc_ptr = alloc_roc_string(core::slice::from_raw_parts(
        buf.offset((offset + 4) as isize),
        len as usize,
    ));
    (offset + 4 + len, roc_ptr)
}

unsafe fn read_roc_string(ptr: i32) -> (*const u8, usize) {
    let len = core::ptr::read_unaligned(ptr as *const i32) as usize;
    (if len > 0 { (ptr + 4) as *const u8 } else { core::ptr::null() }, len)
}

// --- Golem guest exports ---

#[export_name = "golem:agent/guest@1.5.0#initialize"]
pub extern "C" fn golem_initialize(input_ptr: i32) -> i32 {
    unsafe {
        let buf = input_ptr as *const u8;
        let (_off, agent_type) = canon_string_to_roc(buf, 0);
        let input_roc = alloc_roc_string(b"{}");
        let state_out = alloc_roc_string(&[0u8; STATE_BUF_SIZE]);

        roc_golem_initialize(agent_type, input_roc, state_out);

        let (state_data, state_len) = read_roc_string(state_out);
        if state_len > 0 && state_len <= STATE_BUF_SIZE {
            core::ptr::copy_nonoverlapping(state_data, STATE_BUF.as_mut_ptr(), state_len);
            STATE_LEN = state_len;
        }

        let out = alloc(16, 1) as *mut u8;
        encode_result_ok_empty(out, 0);
        out as i32
    }
}

#[export_name = "golem:agent/guest@1.5.0#invoke"]
pub extern "C" fn golem_invoke(input_ptr: i32) -> i32 {
    unsafe {
        let buf = input_ptr as *const u8;
        let (_off, method_name) = canon_string_to_roc(buf, 0);
        let input_roc = alloc_roc_string(b"{}");

        let state_ptr = if STATE_LEN > 0 {
            alloc_roc_string(core::slice::from_raw_parts(STATE_BUF.as_ptr(), STATE_LEN))
        } else {
            alloc_roc_string(b"{}")
        };

        let output = alloc_roc_string(&[0u8; 4096]);
        roc_golem_invoke(method_name, state_ptr, input_roc, output);

        let (result_data, result_len) = read_roc_string(output);
        if result_len > 0 && result_len <= STATE_BUF_SIZE {
            core::ptr::copy_nonoverlapping(result_data, STATE_BUF.as_mut_ptr(), result_len);
            STATE_LEN = result_len;
        }

        let out = alloc(32, 1) as *mut u8;
        encode_result_ok_data_value_empty(out, 0);
        out as i32
    }
}

#[export_name = "golem:agent/guest@1.5.0#get-definition"]
pub extern "C" fn golem_get_definition() -> i32 {
    unsafe {
        let output = alloc_roc_string(&[0u8; 4096]);
        roc_golem_get_definition(output);
        let roc_len = core::ptr::read_unaligned(output as *const i32) as usize;
        let roc_data = if roc_len > 0 {
            core::slice::from_raw_parts((output + 4) as *const u8, roc_len)
        } else {
            &[]
        };
        let json_str = core::str::from_utf8_unchecked(roc_data);
        // Roc returns JSON like { typeName, description, ... }
        // For MVP: extract typeName from JSON and build minimal WIT agent-type
        let type_name = if json_str.len() > 10 {
            extract_json_string_field(json_str, "typeName")
        } else {
            "unknown"
        };
        let canon_buf = alloc(1024, 4) as *mut u8;
        encode_minimal_agent_type(canon_buf, 0, type_name);
        canon_buf as i32
    }
}

unsafe fn extract_json_string_field<'a>(json: &'a str, field: &str) -> &'a str {
    // Manual byte-by-byte scan to avoid memcmp imports
    let json_bytes = json.as_bytes();
    let field_bytes = field.as_bytes();
    if field_bytes.is_empty() || json_bytes.len() < field_bytes.len() + 6 {
        return "unknown";
    }
    let quote_colon_space: &[u8] = b"\": \"";
    let json_len = json_bytes.len();
    let field_len = field_bytes.len();
    // We look for: "fieldName": "
    for i in 0..(json_len.saturating_sub(field_len + 6)) {
        if json_bytes[i] != b'"' { continue; }
        // Compare field name byte by byte
        let mut match_field = true;
        for k in 0..field_len {
            if json_bytes[i + 1 + k] != field_bytes[k] { match_field = false; break; }
        }
        if !match_field { continue; }
        // Check for ": "
        let colon_start = i + 1 + field_len;
        let mut match_colon = true;
        for k in 0..4 {
            if json_bytes[colon_start + k] != quote_colon_space[k] { match_colon = false; break; }
        }
        if !match_colon { continue; }
        // Found it — value starts after "\": \""
        let value_start = colon_start + 4;
        if value_start >= json_len { break; }
        for j in value_start..json_len {
            if json_bytes[j] == b'"' {
                return core::str::from_utf8_unchecked(&json_bytes[value_start..j]);
            }
        }
        break;
    }
    "unknown"
}

#[export_name = "golem:agent/guest@1.5.0#discover-agent-types"]
pub extern "C" fn golem_discover_agent_types() -> i32 {
    unsafe {
        let output = alloc_roc_string(&[0u8; 4096]);
        roc_golem_discover_types(output);
        let out = alloc(16, 1) as *mut u8;
        encode_result_ok_empty_agent_type_list(out, 0);
        out as i32
    }
}

#[export_name = "golem:api/save-snapshot@1.5.0#save"]
pub extern "C" fn golem_save() -> i32 {
    unsafe {
        let state_ptr = alloc_roc_string(core::slice::from_raw_parts(STATE_BUF.as_ptr(), STATE_LEN));
        let output = alloc_roc_string(&[0u8; 4096]);
        roc_golem_save(state_ptr, output);

        let (result_data, result_len) = read_roc_string(output);
        let out = alloc(1024, 1) as *mut u8;
        encode_snapshot(out, 0, core::slice::from_raw_parts(result_data, result_len), "application/json");
        out as i32
    }
}

#[export_name = "golem:api/load-snapshot@1.5.0#load"]
pub extern "C" fn golem_load(t1: i32, t2: i32, _t3: i32, _t4: i32) -> i32 {
    unsafe {
        let payload = core::slice::from_raw_parts(t1 as *const u8, t2 as usize);
        let snapshot_ptr = alloc_roc_string(payload);
        let output = alloc_roc_string(&[0u8; STATE_BUF_SIZE]);
        roc_golem_load(snapshot_ptr, output);

        let (state_data, state_len) = read_roc_string(output);
        if state_len <= STATE_BUF_SIZE {
            core::ptr::copy_nonoverlapping(state_data, STATE_BUF.as_mut_ptr(), state_len);
            STATE_LEN = state_len;
        }

        let out = alloc(16, 1) as *mut u8;
        encode_result_ok_unit(out, 0);
        out as i32
    }
}

fn cabi_post_noop(_ptr: i32) {}

#[export_name = "cabi_post_golem:agent/guest@1.5.0#initialize"]
pub extern "C" fn golem_cabi_post_initialize(ptr: i32) { cabi_post_noop(ptr) }
#[export_name = "cabi_post_golem:agent/guest@1.5.0#invoke"]
pub extern "C" fn golem_cabi_post_invoke(ptr: i32) { cabi_post_noop(ptr) }
#[export_name = "cabi_post_golem:agent/guest@1.5.0#get-definition"]
pub extern "C" fn golem_cabi_post_get_definition(ptr: i32) { cabi_post_noop(ptr) }
#[export_name = "cabi_post_golem:agent/guest@1.5.0#discover-agent-types"]
pub extern "C" fn golem_cabi_post_discover_agent_types(ptr: i32) { cabi_post_noop(ptr) }
#[export_name = "cabi_post_golem:api/save-snapshot@1.5.0#save"]
pub extern "C" fn golem_cabi_post_save(ptr: i32) { cabi_post_noop(ptr) }
#[export_name = "cabi_post_golem:api/load-snapshot@1.5.0#load"]
pub extern "C" fn golem_cabi_post_load(ptr: i32) { cabi_post_noop(ptr) }

#[export_name = "roc_alloc"]
pub extern "C" fn roc_alloc(size: i32, align: i32) -> i32 {
    cabi_realloc(core::ptr::null_mut(), 0, align as usize, size as usize) as i32
}

#[export_name = "roc_realloc"]
pub extern "C" fn roc_realloc(old_ptr: i32, new_size: i32, align: i32) -> i32 {
    let _ = old_ptr;
    cabi_realloc(core::ptr::null_mut(), 0, align as usize, new_size as usize) as i32
}

#[export_name = "roc_dealloc"]
pub extern "C" fn roc_dealloc(_ptr: i32, _len: i32) {}

#[export_name = "roc_crashed"]
pub extern "C" fn roc_crashed(_msg_ptr: i32, _msg_len: i32) {
    loop {}
}

#[export_name = "__multi3"]
pub extern "C" fn __multi3(result: i32, a_lo: i64, a_hi: i64, b_lo: i64, b_hi: i64) {
    let a = (a_hi as u128) << 64 | (a_lo as u128);
    let b = (b_hi as u128) << 64 | (b_lo as u128);
    let r = a.wrapping_mul(b);
    unsafe {
        core::ptr::write_unaligned(result as *mut i64, r as i64);
        core::ptr::write_unaligned((result + 8) as *mut i64, (r >> 64) as i64);
    }
}

#[export_name = "memcmp"]
pub extern "C" fn memcmp_ptr(s1: *const u8, s2: *const u8, n: usize) -> i32 {
    unsafe {
        for i in 0..n {
            let a = *s1.add(i);
            let b = *s2.add(i);
            if a != b {
                return (a as i32) - (b as i32);
            }
        }
    }
    0
}

#[panic_handler]
fn panic(_: &core::panic::PanicInfo) -> ! {
    loop {}
}
