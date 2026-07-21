#![no_std]

use core::arch::wasm32;
use core::sync::atomic::{AtomicUsize, Ordering};

const HEAP_START: usize = 0x10000;
static ALLOC_OFFSET: AtomicUsize = AtomicUsize::new(HEAP_START);

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
    fn roc_initialize(agent_type_ptr: i32, input_ptr: i32) -> i32;
    fn roc_invoke(method_ptr: i32, input_ptr: i32, output_ptr: i32);
    fn roc_discover_types(output_ptr: i32);
    fn roc_save(output_ptr: i32);
    fn roc_load(snapshot_ptr: i32) -> i32;
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

/// result<_, agent-error> = Ok(())
unsafe fn encode_result_ok_empty(buf: *mut u8, off: i32) -> i32 {
    write_variant_disc(buf, off, 0)
}

/// result<_, agent-error> = Err(invalid-input(""))
unsafe fn encode_result_err_generic(buf: *mut u8, off: i32) -> i32 {
    let off = write_variant_disc(buf, off, 1);
    // agent-error = variant:
    //   0: invalid-input(string)
    //   1: invalid-method(string)
    //   2: invalid-type(string)
    //   3: invalid-agent-id(string)
    //   4: custom-error(value-and-type)
    let off = write_variant_disc(buf, off, 0); // invalid-input
    write_str(buf, off, "roc error")
}

/// result<data-value, agent-error> = Ok(tuple([]))
unsafe fn encode_result_ok_data_value_empty(buf: *mut u8, off: i32) -> i32 {
    let off = write_variant_disc(buf, off, 0); // Ok
    // data-value = variant { tuple(list<element-value>), multimodal(...) }
    //   tuple([]) = disc 0, empty list
    let off = write_variant_disc(buf, off, 0); // tuple
    write_empty_list(buf, off) // empty element-value list
}

/// result<list<agent-type>, agent-error> = Ok([])
unsafe fn encode_result_ok_empty_agent_type_list(buf: *mut u8, off: i32) -> i32 {
    let off = write_variant_disc(buf, off, 0); // Ok
    write_empty_list(buf, off) // empty agent-type list
}

/// result<_, string> = Ok(())
unsafe fn encode_result_ok_unit(buf: *mut u8, off: i32) -> i32 {
    write_variant_disc(buf, off, 0)
}

/// result<_, string> = Err(msg)
unsafe fn encode_result_err_string(buf: *mut u8, off: i32, msg: &str) -> i32 {
    let off = write_variant_disc(buf, off, 1); // Err
    write_str(buf, off, msg)
}

/// snapshot { payload: list<u8>, mime-type: string }
unsafe fn encode_snapshot(buf: *mut u8, off: i32, payload: &[u8], mime: &str) -> i32 {
    // payload: list<u8>
    let off = write_i32(buf, off, payload.len() as i32);
    if !payload.is_empty() {
        core::ptr::copy_nonoverlapping(payload.as_ptr(), buf.offset(off as isize), payload.len());
    }
    let off = off + payload.len() as i32;
    // mime-type: string
    write_str(buf, off, mime)
}

/// Encode a minimal agent-type into canonical ABI buffer.
unsafe fn encode_minimal_agent_type(buf: *mut u8, off: i32, type_name: &str) -> i32 {
    let mut o = off;
    o = write_str(buf, o, type_name);
    o = write_str(buf, o, "");
    o = write_str(buf, o, "roc");
    // constructor: agent-constructor { name: option<string>, description: string, prompt-hint: option<string>, input-schema: data-schema }
    o = write_option_none(buf, o);
    o = write_str(buf, o, "");
    o = write_option_none(buf, o);
    // input-schema: data-schema = tuple([])
    o = write_variant_disc(buf, o, 0); // tuple
    o = write_empty_list(buf, o);
    // methods, dependencies: empty lists
    o = write_empty_list(buf, o);
    o = write_empty_list(buf, o);
    // mode: agent-mode = ephemeral
    o = write_enum(buf, o, 1);
    // http-mount: none
    o = write_option_none(buf, o);
    // snapshotting: enabled(default)
    o = write_variant_disc(buf, o, 1); // enabled
    o = write_variant_disc(buf, o, 0); // default
    // config: empty list
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

// --- Golem guest exports ---

#[export_name = "golem:agent/guest@1.5.0#initialize"]
pub extern "C" fn golem_initialize(input_ptr: i32) -> i32 {
    unsafe {
        let buf = input_ptr as *const u8;
        let (_next, agent_type) = canon_string_to_roc(buf, 0);
        let result = roc_initialize(agent_type, _next);
        let out = alloc(16, 1) as *mut u8;
        if result == 0 {
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
        let (_next, method_name) = canon_string_to_roc(buf, 0);
        let output = alloc_roc_string(&[0u8; 4096]);
        let dummy_input = alloc_roc_string(b"{}");
        roc_invoke(method_name, dummy_input, output);
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
        let output = alloc_roc_string(&[0u8; 4096]);
        roc_save(output);
        let roc_len = core::ptr::read_unaligned(output as *const i32) as usize;
        let roc_data = if roc_len > 0 {
            core::slice::from_raw_parts((output + 4) as *const u8, roc_len)
        } else {
            &[]
        };
        let out = alloc(1024, 1) as *mut u8;
        encode_snapshot(out, 0, roc_data, "application/json");
        out as i32
    }
}

#[export_name = "golem:api/load-snapshot@1.5.0#load"]
pub extern "C" fn golem_load(t1: i32, _t2: i32, _t3: i32, _t4: i32) -> i32 {
    unsafe {
        let result = roc_load(t1);
        let out = alloc(16, 1) as *mut u8;
        if result == 0 {
            encode_result_ok_unit(out, 0);
        } else {
            encode_result_err_string(out, 0, "load failed");
        }
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
