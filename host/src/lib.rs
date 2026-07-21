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
    fn roc_get_agent_type(input_ptr: i32, output_ptr: i32);
    fn roc_initialize(agent_type_ptr: i32, input_ptr: i32, state_output_ptr: i32);
    fn roc_invoke(method_ptr: i32, state_ptr: i32, input_ptr: i32, output_ptr: i32);
    fn roc_discover_types(output_ptr: i32);
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
    let off = write_variant_disc(buf, off, 0); // invalid-input
    write_str(buf, off, "roc error")
}

unsafe fn encode_result_ok_data_value_empty(buf: *mut u8, off: i32) -> i32 {
    let off = write_variant_disc(buf, off, 0); // Ok
    let off = write_variant_disc(buf, off, 0); // tuple
    write_empty_list(buf, off)
}

unsafe fn encode_result_ok_empty_agent_type_list(buf: *mut u8, off: i32) -> i32 {
    let off = write_variant_disc(buf, off, 0); // Ok
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
    o = write_variant_disc(buf, o, 0); // tuple
    o = write_empty_list(buf, o);
    o = write_empty_list(buf, o);
    o = write_empty_list(buf, o);
    o = write_enum(buf, o, 1); // ephemeral
    o = write_option_none(buf, o);
    o = write_variant_disc(buf, o, 1); // enabled
    o = write_variant_disc(buf, o, 0); // default
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

/// Read a canon-ABI string (i32 len + data) from buf, create a Roc string, return (next_off, roc_ptr)
unsafe fn canon_string_to_roc(buf: *const u8, offset: i32) -> (i32, i32) {
    let len = core::ptr::read_unaligned(buf.offset(offset as isize) as *const i32);
    let roc_ptr = alloc_roc_string(core::slice::from_raw_parts(
        buf.offset((offset + 4) as isize),
        len as usize,
    ));
    (offset + 4 + len, roc_ptr)
}

/// Read a Roc string from a pointer (len-prefixed), return (pointer_to_data, length)
unsafe fn read_roc_string(ptr: i32) -> (*const u8, usize) {
    let len = core::ptr::read_unaligned(ptr as *const i32) as usize;
    (if len > 0 { (ptr + 4) as *const u8 } else { core::ptr::null() }, len)
}

// --- Golem guest exports ---

#[export_name = "golem:agent/guest@1.5.0#initialize"]
pub extern "C" fn golem_initialize(input_ptr: i32) -> i32 {
    unsafe {
        let buf = input_ptr as *const u8;

        // Read agent type name from WIT-encoded input: (agent-type, data-value, principal)
        let (_off, agent_type) = canon_string_to_roc(buf, 0);

        // Pass empty input for MVP — data-value parsing is complex (variant of variant)
        let input_roc = alloc_roc_string(b"{}");

        // Pre-allocate output buffer for Roc to write initial state
        let state_out = alloc_roc_string(&[0u8; STATE_BUF_SIZE]);

        // Call Roc's initialize! — writes initial state to state_out
        roc_initialize(agent_type, input_roc, state_out);

        // Read initial state from Roc's output
        let (state_data, state_len) = read_roc_string(state_out);
        let success = state_len > 0 && state_len <= STATE_BUF_SIZE;
        let out = alloc(16, 1) as *mut u8;
        if success {
            core::ptr::copy_nonoverlapping(state_data, STATE_BUF.as_mut_ptr(), state_len);
            STATE_LEN = state_len;
            encode_result_ok_empty(out, 0);
        } else {
            encode_result_err_generic(out, 0);
        }
        out as i32
    }
}

#[export_name = "golem:agent/guest@1.5.0#invoke"]
pub extern "C" fn golem_invoke(input_ptr: i32) -> i32 {
    unsafe {
        let buf = input_ptr as *const u8;

        // Read method name from WIT-encoded input: (method-name, data-value, principal)
        let (_off, method_name) = canon_string_to_roc(buf, 0);

        // Pass empty input for MVP — data-value parsing is complex
        let input_roc = alloc_roc_string(b"{}");

        // Write current state from buffer to linear memory as Roc string
        let state_ptr = if STATE_LEN > 0 {
            alloc_roc_string(core::slice::from_raw_parts(
                STATE_BUF.as_ptr(),
                STATE_LEN,
            ))
        } else {
            alloc_roc_string(b"{}")
        };

        // Pre-allocate output buffer for Roc to write result
        let output = alloc_roc_string(&[0u8; 4096]);

        // Call Roc's invoke! — dispatches to handler, writes result to output
        roc_invoke(method_name, state_ptr, input_roc, output);

        // Read result from output buffer — becomes new state
        let (result_data, result_len) = read_roc_string(output);
        if result_len > 0 && result_len <= STATE_BUF_SIZE {
            core::ptr::copy_nonoverlapping(result_data, STATE_BUF.as_mut_ptr(), result_len);
            STATE_LEN = result_len;
        }

        // Encode result as result<data-value, agent-error> with empty data-value
        let out = alloc(32, 1) as *mut u8;
        encode_result_ok_data_value_empty(out, 0)
    }
}

#[export_name = "golem:agent/guest@1.5.0#get-definition"]
pub extern "C" fn golem_get_definition() -> i32 {
    unsafe {
        let output = alloc_roc_string(&[0u8; 4096]);
        roc_get_agent_type(0, output);
        let roc_len = core::ptr::read_unaligned(output as *const i32) as usize;
        let roc_data = if roc_len > 0 {
            core::slice::from_raw_parts((output + 4) as *const u8, roc_len)
        } else {
            &[]
        };
        let type_name = core::str::from_utf8_unchecked(roc_data);
        let canon_buf = alloc(1024, 4) as *mut u8;
        encode_minimal_agent_type(canon_buf, 0, type_name);
        canon_buf as i32
    }
}

#[export_name = "golem:agent/guest@1.5.0#discover-agent-types"]
pub extern "C" fn golem_discover_agent_types() -> i32 {
    unsafe {
        let output = alloc_roc_string(&[0u8; 4096]);
        roc_discover_types(output);
        let out = alloc(16, 1) as *mut u8;
        encode_result_ok_empty_agent_type_list(out, 0);
        out as i32
    }
}

#[export_name = "golem:api/save-snapshot@1.5.0#save"]
pub extern "C" fn golem_save() -> i32 {
    unsafe {
        let payload = core::slice::from_raw_parts(STATE_BUF.as_ptr(), STATE_LEN);
        let out = alloc(1024, 1) as *mut u8;
        encode_snapshot(out, 0, payload, "application/json");
        out as i32
    }
}

#[export_name = "golem:api/load-snapshot@1.5.0#load"]
pub extern "C" fn golem_load(t1: i32, t2: i32, _t3: i32, _t4: i32) -> i32 {
    unsafe {
        // t1 = payload pointer, t2 = payload length
        let payload = core::slice::from_raw_parts(t1 as *const u8, t2 as usize);
        let len = payload.len();
        if len <= STATE_BUF_SIZE {
            core::ptr::copy_nonoverlapping(payload.as_ptr(), STATE_BUF.as_mut_ptr(), len);
            STATE_LEN = len;
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

#[export_name = "roc_dealloc"]
pub extern "C" fn roc_dealloc(_ptr: i32, _len: i32) {}

#[panic_handler]
fn panic(_: &core::panic::PanicInfo) -> ! {
    loop {}
}
