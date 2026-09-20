use core::alloc::Layout;
use core::fmt;
use core::mem::{self, ManuallyDrop};
use core::ptr;
use core::slice;
use core::str;

use std::alloc::{alloc, dealloc, realloc};

/// Standard Roc Memory Allocator exported symbols for Roc ABI
#[no_mangle]
pub unsafe extern "C" fn roc_alloc(size: usize, alignment: u32) -> *mut u8 {
    let layout = Layout::from_size_align_unchecked(size, alignment as usize);
    alloc(layout)
}

#[no_mangle]
pub unsafe extern "C" fn roc_realloc(
    c_ptr: *mut u8,
    new_size: usize,
    old_size: usize,
    alignment: u32,
) -> *mut u8 {
    let old_layout = Layout::from_size_align_unchecked(old_size, alignment as usize);
    realloc(c_ptr, old_layout, new_size)
}

#[no_mangle]
pub unsafe extern "C" fn roc_dealloc(c_ptr: *mut u8, alignment: u32) {
    let layout = Layout::from_size_align_unchecked(1, alignment as usize);
    dealloc(c_ptr, layout);
}

#[no_mangle]
pub unsafe extern "C" fn roc_panic(msg: *mut RocStr, _tag_id: u32) {
    let msg_str = if msg.is_null() {
        "Unknown Roc panic"
    } else {
        (*msg).as_str()
    };
    panic!("Roc guest panic: {}", msg_str);
}

#[no_mangle]
pub unsafe extern "C" fn roc_dbg(_loc: *mut RocStr, msg: *mut RocStr, _src: *mut RocStr) {
    if !msg.is_null() {
        let m = (*msg).as_str();
        eprintln!("[ROC_DBG] {}", m);
    }
}

#[no_mangle]
pub unsafe extern "C" fn roc_memset(dst: *mut u8, c: i32, length: usize) -> *mut u8 {
    ptr::write_bytes(dst, c as u8, length);
    dst
}

#[no_mangle]
pub unsafe extern "C" fn roc_memcpy(dst: *mut u8, src: *const u8, length: usize) -> *mut u8 {
    ptr::copy_nonoverlapping(src, dst, length);
    dst
}

/// Roc String (RocStr) supporting Small String Optimization (SSO) and heap refcounted strings.
#[repr(C)]
pub struct RocStr {
    words: [usize; 3],
}

impl RocStr {
    pub const SIZE: usize = mem::size_of::<usize>() * 3;
    const MASK: u8 = 0x80;

    pub fn empty() -> Self {
        Self { words: [0; 3] }
    }

    fn as_bytes(&self) -> &[u8] {
        unsafe { slice::from_raw_parts(self.words.as_ptr() as *const u8, Self::SIZE) }
    }

    fn as_bytes_mut(&mut self) -> &mut [u8] {
        unsafe { slice::from_raw_parts_mut(self.words.as_mut_ptr() as *mut u8, Self::SIZE) }
    }

    pub fn from_str(s: &str) -> Self {
        let len = s.len();
        if len < Self::SIZE {
            let mut roc_str = Self::empty();
            let bytes = roc_str.as_bytes_mut();
            bytes[..len].copy_from_slice(s.as_bytes());
            bytes[Self::SIZE - 1] = (len as u8) | Self::MASK;
            roc_str
        } else {
            let align = mem::align_of::<usize>();
            let header_size = mem::size_of::<isize>();
            let total_size = header_size + len;
            unsafe {
                let layout = Layout::from_size_align_unchecked(total_size, align);
                let ptr = alloc(layout);
                if ptr.is_null() {
                    std::alloc::handle_alloc_error(layout);
                }
                *(ptr as *mut isize) = 1;
                let data_ptr = ptr.add(header_size);
                ptr::copy_nonoverlapping(s.as_ptr(), data_ptr, len);

                let mut roc_str = Self::empty();
                roc_str.words[0] = data_ptr as usize;
                roc_str.words[1] = len;
                roc_str.words[2] = len; // capacity
                roc_str
            }
        }
    }

    pub fn is_small(&self) -> bool {
        (self.as_bytes()[Self::SIZE - 1] & Self::MASK) != 0
    }

    pub fn as_slice(&self) -> &[u8] {
        if self.is_small() {
            let len = (self.as_bytes()[Self::SIZE - 1] & !Self::MASK) as usize;
            &self.as_bytes()[..len]
        } else {
            unsafe {
                let ptr = self.words[0] as *const u8;
                let len = self.words[1];
                if ptr.is_null() || len == 0 {
                    &[]
                } else {
                    slice::from_raw_parts(ptr, len)
                }
            }
        }
    }

    pub fn as_str(&self) -> &str {
        str::from_utf8(self.as_slice()).unwrap_or("")
    }

    pub fn to_string(&self) -> String {
        String::from(self.as_str())
    }
}

impl Clone for RocStr {
    fn clone(&self) -> Self {
        if self.is_small() {
            Self { words: self.words }
        } else {
            Self::from_str(self.as_str())
        }
    }
}

impl Drop for RocStr {
    fn drop(&mut self) {
        if !self.is_small() {
            let data_ptr = self.words[0] as *mut u8;
            let capacity = self.words[2];
            if !data_ptr.is_null() && capacity > 0 {
                unsafe {
                    let header_size = mem::size_of::<isize>();
                    let ptr = data_ptr.sub(header_size);
                    let ref_count = ptr as *mut isize;
                    *ref_count -= 1;
                    if *ref_count <= 0 {
                        let total_size = header_size + capacity;
                        let layout =
                            Layout::from_size_align_unchecked(total_size, mem::align_of::<usize>());
                        dealloc(ptr, layout);
                    }
                }
            }
        }
    }
}

impl fmt::Display for RocStr {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

impl fmt::Debug for RocStr {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:?}", self.as_str())
    }
}

/// Roc List (RocList<T>)
#[repr(C)]
pub struct RocList<T> {
    ptr: *mut T,
    len: usize,
    capacity: usize,
}

impl<T> RocList<T> {
    pub fn empty() -> Self {
        Self {
            ptr: ptr::null_mut(),
            len: 0,
            capacity: 0,
        }
    }

    pub fn from_vec(mut v: Vec<T>) -> Self {
        let len = v.len();
        let capacity = v.capacity();
        let ptr = v.as_mut_ptr();
        mem::forget(v);
        Self { ptr, len, capacity }
    }

    pub fn as_slice(&self) -> &[T] {
        if self.ptr.is_null() || self.len == 0 {
            &[]
        } else {
            unsafe { slice::from_raw_parts(self.ptr, self.len) }
        }
    }

    pub fn len(&self) -> usize {
        self.len
    }

    pub fn is_empty(&self) -> bool {
        self.len == 0
    }
}

impl<T> Drop for RocList<T> {
    fn drop(&mut self) {
        if !self.ptr.is_null() && self.capacity > 0 {
            unsafe {
                let _ = Vec::from_raw_parts(self.ptr, self.len, self.capacity);
            }
        }
    }
}

/// Roc Result (RocResult<T, E>) layout
#[repr(C)]
pub struct RocResult<T, E> {
    payload: RocResultPayload<T, E>,
    discriminant: u8,
}

#[repr(C)]
union RocResultPayload<T, E> {
    ok: ManuallyDrop<T>,
    err: ManuallyDrop<E>,
}

impl<T, E> RocResult<T, E> {
    pub fn ok(value: T) -> Self {
        Self {
            payload: RocResultPayload {
                ok: ManuallyDrop::new(value),
            },
            discriminant: 0,
        }
    }

    pub fn err(error: E) -> Self {
        Self {
            payload: RocResultPayload {
                err: ManuallyDrop::new(error),
            },
            discriminant: 1,
        }
    }

    pub fn is_ok(&self) -> bool {
        self.discriminant == 0
    }

    pub fn is_err(&self) -> bool {
        self.discriminant != 0
    }

    pub fn into_result(mut self) -> Result<T, E> {
        if self.discriminant == 0 {
            let ok = unsafe { ManuallyDrop::take(&mut self.payload.ok) };
            mem::forget(self);
            Ok(ok)
        } else {
            let err = unsafe { ManuallyDrop::take(&mut self.payload.err) };
            mem::forget(self);
            Err(err)
        }
    }
}

impl<T, E> Drop for RocResult<T, E> {
    fn drop(&mut self) {
        if self.discriminant == 0 {
            unsafe { ManuallyDrop::drop(&mut self.payload.ok) };
        } else {
            unsafe { ManuallyDrop::drop(&mut self.payload.err) };
        }
    }
}
