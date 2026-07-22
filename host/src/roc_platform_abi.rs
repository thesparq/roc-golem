#![cfg_attr(rustfmt, rustfmt_skip)]
#![allow(dead_code)]

use core::ffi::c_void;

#[repr(C)]
#[derive(Clone, Copy)]
pub struct RocDec {
    pub num: i128,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct RocStr {
    pub bytes: *mut u8,
    pub capacity_or_alloc_ptr: usize,
    pub length: usize,
}

impl RocStr {
    pub fn empty() -> Self {
        Self { bytes: core::ptr::null_mut(), capacity_or_alloc_ptr: 0, length: 0 }
    }
}

pub type RocBox = *mut c_void;

#[repr(C)]
pub struct RocHost {
    pub env: *mut c_void,
    pub roc_alloc: extern "C" fn(*mut RocHost, usize, usize) -> *mut c_void,
    pub roc_dealloc: extern "C" fn(*mut RocHost, *mut c_void, usize),
    pub roc_realloc: extern "C" fn(*mut RocHost, *mut c_void, usize, usize) -> *mut c_void,
    pub roc_dbg: extern "C" fn(*mut RocHost, *const u8, usize),
    pub roc_expect_failed: extern "C" fn(*mut RocHost, *const u8, usize),
    pub roc_crashed: extern "C" fn(*mut RocHost, *const u8, usize),
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct RocList<T> {
    pub data: *mut T,
    pub len: usize,
    pub capacity: usize,
}

extern "C" {
    pub fn roc_golem_initialize(agent_type_ptr: i32, input_ptr: i32, state_output_ptr: i32);
    pub fn roc_golem_invoke(ret_area: i32, method_ptr: i32, state_ptr: i32, input_ptr: i32, output_ptr: i32);
    pub fn roc_golem_get_definition(output_ptr: i32);
    pub fn roc_golem_discover_types(output_ptr: i32);
    pub fn roc_golem_save(state_ptr: i32, output_ptr: i32);
    pub fn roc_golem_load(snapshot_ptr: i32, state_output_ptr: i32);
    pub fn roc_alloc(size: i32, align: i32) -> i32;
    pub fn roc_realloc(old_ptr: i32, new_size: i32, align: i32) -> i32;
    pub fn roc_dealloc(ptr: i32, len: i32);
    pub fn roc_crashed(msg_ptr: i32, msg_len: i32);
    pub fn roc_dbg(msg_ptr: i32, msg_len: i32);
}
