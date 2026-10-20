use core::alloc::Layout;
use core::fmt;
use core::mem::{self, ManuallyDrop};
use core::ptr;
use core::slice;
use core::str;

use std::alloc::{alloc, dealloc, realloc};

const ALLOC_HEADER_SIZE: usize = 4 * mem::size_of::<usize>();

/// Standard Roc Memory Allocator exported symbols for Roc ABI.
///
/// A 16-byte allocation header (4 words) keeps the Rust allocator's `total_size`
/// word, stored at `ptr - ALLOC_HEADER_SIZE`, clear of everything the runtime
/// writes into the space it asked for. The runtime's own words live *after* the
/// returned pointer: a refcount one word before the bytes
/// (`RocStr.decref` in `src/builtins/str.zig` → `utils.decref_ptr_to_refcount`)
/// and the seamless-slice tag in the string's capacity word. `roc_dealloc` is
/// called with the same pointer `roc_alloc` returned, which for a string is the
/// refcount word rather than the bytes.
#[no_mangle]
pub unsafe extern "C" fn roc_alloc(size: usize, alignment: u32) -> *mut u8 {
    let align = (alignment as usize).max(mem::align_of::<usize>()).max(ALLOC_HEADER_SIZE);
    let header_size = ALLOC_HEADER_SIZE.max(align);
    let total_size = header_size + size;
    let layout = Layout::from_size_align_unchecked(total_size, align);
    let ptr = alloc(layout);
    if ptr.is_null() {
        std::alloc::handle_alloc_error(layout);
    }
    let data_ptr = ptr.add(header_size);
    let total_size_ptr = (data_ptr.sub(ALLOC_HEADER_SIZE)) as *mut usize;
    *total_size_ptr = total_size;
    data_ptr
}

#[no_mangle]
pub unsafe extern "C" fn roc_realloc(
    c_ptr: *mut u8,
    new_size: usize,
    alignment: u32,
) -> *mut u8 {
    if c_ptr.is_null() {
        return roc_alloc(new_size, alignment);
    }
    let align = (alignment as usize).max(mem::align_of::<usize>()).max(ALLOC_HEADER_SIZE);
    let header_size = ALLOC_HEADER_SIZE.max(align);
    let total_size_ptr = (c_ptr.sub(ALLOC_HEADER_SIZE)) as *const usize;
    let old_total = *total_size_ptr;
    let new_total = header_size + new_size;
    let ptr = c_ptr.sub(header_size);
    let old_layout = Layout::from_size_align_unchecked(old_total, align);
    let new_ptr = realloc(ptr, old_layout, new_total);
    if new_ptr.is_null() {
        let new_layout = Layout::from_size_align_unchecked(new_total, align);
        std::alloc::handle_alloc_error(new_layout);
    }
    let data_ptr = new_ptr.add(header_size);
    let new_total_size_ptr = (data_ptr.sub(ALLOC_HEADER_SIZE)) as *mut usize;
    *new_total_size_ptr = new_total;
    data_ptr
}

#[no_mangle]
pub unsafe extern "C" fn roc_dealloc(c_ptr: *mut u8, alignment: u32) {
    if !c_ptr.is_null() {
        let align = (alignment as usize).max(mem::align_of::<usize>()).max(ALLOC_HEADER_SIZE);
        let header_size = ALLOC_HEADER_SIZE.max(align);
        let total_size_ptr = (c_ptr.sub(ALLOC_HEADER_SIZE)) as *const usize;
        let total_size = *total_size_ptr;
        let ptr = c_ptr.sub(header_size);
        let layout = Layout::from_size_align_unchecked(total_size, align);
        dealloc(ptr, layout);
    }
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

/// Roc's runtime reporting hooks. The generated ABI
/// (`tooling/glue/generated/roc_platform_abi.rs`) declares all of them as
/// `(bytes: *const u8, len: usize)`: these used to take two `*mut RocStr`s and
/// ignore them, so `dbg!` output, `expect` failures and crashes were silent.
/// `roc_crashed` must not return — Roc traps right after the call.
unsafe fn log_roc_bytes(level: crate::wasi::logging::logging::Level, prefix: &str, bytes: *const u8, len: usize) {
    let slice = if bytes.is_null() {
        &[][..]
    } else {
        core::slice::from_raw_parts(bytes, len)
    };
    let text = String::from_utf8_lossy(slice);
    crate::wasi::logging::logging::log(level, "roc-agent", &format!("{prefix}{text}"));
}

#[no_mangle]
pub unsafe extern "C" fn roc_crashed(bytes: *const u8, len: usize) {
    log_roc_bytes(
        crate::wasi::logging::logging::Level::Error,
        "Roc crashed: ",
        bytes,
        len,
    );
    // The contract is that this never returns.
    unreachable!("Roc execution crashed");
}

#[no_mangle]
pub unsafe extern "C" fn roc_expect_failed(bytes: *const u8, len: usize) {
    log_roc_bytes(
        crate::wasi::logging::logging::Level::Error,
        "Roc expect failed: ",
        bytes,
        len,
    );
}

#[no_mangle]
pub unsafe extern "C" fn roc_dbg(bytes: *const u8, len: usize) {
    log_roc_bytes(
        crate::wasi::logging::logging::Level::Debug,
        "roc dbg: ",
        bytes,
        len,
    );
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

/// Component-model boundary allocator (`cabi_realloc`).
///
/// wit-bindgen's generated glue builds Rust `String`/`Vec` values straight from
/// these pointers and drops them through Rust's global allocator, so this
/// function must behave like a plain allocation. It must NOT hand out pointers
/// carrying the Roc header that `roc_alloc` reserves: doing so made `initialize`
/// abort inside dlmalloc while dropping the incoming `DataValue`, which then
/// poisoned the whole instance (`wasm unreachable`, and every later call failing
/// with `cannot enter component instance`).
#[no_mangle]
pub unsafe extern "C" fn cabi_realloc(
    old_ptr: *mut u8,
    old_size: usize,
    alignment: usize,
    new_size: usize,
) -> *mut u8 {
    let align = alignment.max(1);
    if old_ptr.is_null() {
        if new_size == 0 {
            return align as *mut u8;
        }
        let layout = Layout::from_size_align_unchecked(new_size, align);
        let ptr = alloc(layout);
        if ptr.is_null() {
            std::alloc::handle_alloc_error(layout);
        }
        ptr
    } else if new_size == 0 {
        let layout = Layout::from_size_align_unchecked(old_size, align);
        dealloc(old_ptr, layout);
        align as *mut u8
    } else {
        let layout = Layout::from_size_align_unchecked(old_size, align);
        let ptr = realloc(old_ptr, layout, new_size);
        if ptr.is_null() {
            let new_layout = Layout::from_size_align_unchecked(new_size, align);
            std::alloc::handle_alloc_error(new_layout);
        }
        ptr
    }
}

/// Roc String (RocStr) supporting Small String Optimization (SSO) and heap refcounted strings.
#[repr(C)]
pub struct RocStr {
    words: [usize; 3],
}

impl RocStr {
    pub const SIZE: usize = mem::size_of::<usize>() * 3;
    const MASK: u8 = 0x80;
    /// `RocStr.alignment` in the compiler's builtins: the refcount and capacity
    /// words are pointer-sized, so the allocation is pointer-aligned.
    const ALIGNMENT: usize = mem::size_of::<usize>();
    /// `SEAMLESS_SLICE_TAG`: a slice keeps the *original* allocation's pointer in
    /// the capacity word, tagged with this bit.
    const SEAMLESS_SLICE_TAG: usize = 1;
    /// `capacity_shift`: capacities are stored shifted left by one so that bit 0
    /// stays free for the slice tag.
    const CAPACITY_SHIFT: usize = 1;
    /// `REFCOUNT_STATIC_DATA`: a string whose data lives for the whole program
    /// (a constant) carries this refcount and must never be freed.
    const REFCOUNT_STATIC_DATA: isize = 0;

    pub fn empty() -> Self {
        Self { words: [0; 3] }
    }

    fn as_bytes(&self) -> &[u8] {
        unsafe { slice::from_raw_parts(self.words.as_ptr() as *const u8, Self::SIZE) }
    }

    fn as_bytes_mut(&mut self) -> &mut [u8] {
        unsafe { slice::from_raw_parts_mut(self.words.as_mut_ptr() as *mut u8, Self::SIZE) }
    }

    /// The bytes of the string, following Roc's `RocStr.getAllocationPtr`: the
    /// bytes pointer for a plain heap string, and the original string's bytes
    /// for a seamless slice. `None` for small strings and for the empty string.
    fn data_ptr(&self) -> Option<*mut u8> {
        if self.is_small() {
            return None;
        }
        let capacity_or_alloc_ptr = self.words[1];
        let ptr = if capacity_or_alloc_ptr & Self::SEAMLESS_SLICE_TAG == Self::SEAMLESS_SLICE_TAG
        {
            capacity_or_alloc_ptr & !Self::SEAMLESS_SLICE_TAG
        } else {
            self.words[0]
        };
        if ptr == 0 {
            None
        } else {
            Some(ptr as *mut u8)
        }
    }

    /// Where the runtime keeps a heap string's refcount: the word immediately
    /// before the bytes (`utils.decref_ptr_to_refcount`).
    fn refcount_ptr(&self) -> Option<*mut isize> {
        self.data_ptr()
            .map(|data_ptr| unsafe { (data_ptr as *mut isize).sub(1) })
    }

    /// The pointer the runtime hands back to the host's `roc_dealloc` for a heap
    /// string: `utils.free_ptr_to_refcount` walks back from the refcount by
    /// `max(required_space, alignment) - word`, which for a string is the
    /// pointer-sized gap that holds the refcount itself.
    fn allocation_ptr(&self) -> Option<*mut u8> {
        self.data_ptr()
            .map(|data_ptr| unsafe { data_ptr.sub(Self::ALIGNMENT) })
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
            unsafe {
                // Mirror the compiler's `RocStr.allocate`: it asks for the data
                // plus one word for the refcount and keeps the refcount in that
                // word, immediately before the bytes. Our `roc_alloc` reserves
                // its own size header before all of it.
                let extra_bytes = Self::ALIGNMENT;
                let new_bytes = roc_alloc(extra_bytes + len, Self::ALIGNMENT as u32);
                let data_ptr = new_bytes.add(extra_bytes);
                *((data_ptr as *mut isize).sub(1)) = 1;

                ptr::copy_nonoverlapping(s.as_ptr(), data_ptr, len);

                let mut roc_str = Self::empty();
                roc_str.words[0] = data_ptr as usize;
                roc_str.words[1] = len << Self::CAPACITY_SHIFT; // capacity
                roc_str.words[2] = len;
                roc_str
            }
        }
    }

    /// The raw words, for the cross-check test against the generated ABI.
    #[cfg(test)]
    pub fn words(&self) -> [usize; 3] {
        self.words
    }

    /// Builds a string from raw words, for the cross-check test.
    #[cfg(test)]
    pub fn from_parts(bytes: *mut u8, capacity_or_alloc_ptr: usize, length: usize) -> Self {
        Self {
            words: [bytes as usize, capacity_or_alloc_ptr, length],
        }
    }

    /// The allocation a string owns, for the cross-check test.
    #[cfg(test)]
    pub fn allocation_ptr_for_test(&self) -> *mut u8 {
        self.data_ptr().unwrap_or(core::ptr::null_mut())
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
                let len = self.words[2];
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
            // Shares the allocation, the way the runtime's `incref` does: the
            // refcount is the word before the allocation, and a slice counts
            // against the allocation it points into.
            if let Some(refcount_ptr) = self.refcount_ptr() {
                unsafe {
                    if *refcount_ptr > 0 {
                        *refcount_ptr += 1;
                        return Self { words: self.words };
                    }
                }
            }
            Self::from_str(self.as_str())
        }
    }
}

impl Drop for RocStr {
    /// Releases the string the way the compiler's runtime does
    /// (`RocStr.decref` → `utils.decref`): the refcount is the word immediately
    /// before the bytes, a seamless slice counts against the allocation its
    /// bytes point into, the host's `roc_dealloc` gets the allocation pointer
    /// (not the bytes), and a refcount of zero means the data has whole-program
    /// lifetime.
    fn drop(&mut self) {
        let Some(refcount_ptr) = self.refcount_ptr() else {
            return;
        };
        unsafe {
            let refcount = *refcount_ptr;
            if refcount > 1 {
                *refcount_ptr = refcount - 1;
            } else if refcount == 1 {
                *refcount_ptr = Self::REFCOUNT_STATIC_DATA;
                if let Some(alloc_ptr) = self.allocation_ptr() {
                    roc_dealloc(alloc_ptr, Self::ALIGNMENT as u32);
                }
            }
            // Anything else (zero for a constant, or a poisoned debug value)
            // is not ours to free.
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
            discriminant: 1,
        }
    }

    pub fn err(error: E) -> Self {
        Self {
            payload: RocResultPayload {
                err: ManuallyDrop::new(error),
            },
            discriminant: 0,
        }
    }

    pub fn is_ok(&self) -> bool {
        self.discriminant == 1
    }

    pub fn is_err(&self) -> bool {
        self.discriminant != 1
    }

    pub fn into_result(mut self) -> Result<T, E> {
        if self.discriminant == 1 {
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
        if self.discriminant == 1 {
            unsafe { ManuallyDrop::drop(&mut self.payload.ok) };
        } else {
            unsafe { ManuallyDrop::drop(&mut self.payload.err) };
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_small_roc_str() {
        let s = "hello";
        let roc_str = RocStr::from_str(s);
        assert!(roc_str.is_small());
        assert_eq!(roc_str.as_str(), s);
        assert_eq!(roc_str.to_string(), s);
    }

    #[test]
    fn test_large_roc_str() {
        let s = "This is a large string that exceeds small string optimization limits in Roc standard library.";
        let roc_str = RocStr::from_str(s);
        assert!(!roc_str.is_small());
        assert_eq!(roc_str.as_str(), s);
        assert_eq!(roc_str.to_string(), s);

        // Verify the compiler's header layout (RocStr in src/builtins/str.zig):
        // - Rust allocator total_size is at (data_ptr - 4*word), our own header
        // - Roc's refcount is the word immediately before the data pointer
        // - Roc's capacity lives in the string's second word, shifted left by
        //   one so bit 0 stays free for the seamless-slice tag
        unsafe {
            let data_ptr = roc_str.words[0] as *const u8;
            let refcount = *((data_ptr as *const isize).sub(1));
            let alloc_ptr = roc_str.allocation_ptr().expect("heap string has an allocation");
            let total_size = *(alloc_ptr.sub(ALLOC_HEADER_SIZE) as *const usize);
            assert_eq!(refcount, 1);
            assert_eq!(roc_str.words[1], s.len() << 1);
            assert_eq!(roc_str.words[2], s.len());
            assert!(total_size >= ALLOC_HEADER_SIZE + s.len());
        }

        let cloned = roc_str.clone();
        assert_eq!(cloned.as_str(), s);
        unsafe {
            let data_ptr = roc_str.words[0] as *const u8;
            let refcount = *((data_ptr as *const isize).sub(1));
            assert_eq!(refcount, 2);
        }
    }

    /// A seamless slice keeps the original allocation's pointer (tagged) in its
    /// capacity word; dropping one must release that allocation, not the
    /// interior pointer.
    #[test]
    fn test_seamless_slice_drop() {
        let text = "a string long enough to live on the heap, so slicing it stays off the stack";
        let original = RocStr::from_str(text);
        let alloc_ptr = original.words[0];

        // `capacity_or_alloc_ptr` = original pointer | SEAMLESS_SLICE_TAG.
        let slice = RocStr {
            words: [alloc_ptr + 8, alloc_ptr | 1, text.len() - 8],
        };
        assert!(!slice.is_small());
        assert_eq!(slice.as_str(), &text[8..]);
        drop(slice);

        unsafe {
            // The slice's drop decremented the shared refcount...
            let refcount = *((alloc_ptr as *const isize).sub(1));
            assert_eq!(refcount, 0);
        }
        // ...and the original's drop freed the allocation it owned, without
        // trapping: a double free would panic inside the allocator.
        drop(original);
    }

    /// A constant's data has whole-program lifetime: refcount zero means the
    /// string must not be freed at all.
    #[test]
    fn test_static_data_is_not_freed() {
        unsafe {
            let data_ptr = roc_alloc(64, 4);
            // A constant carries refcount zero; the allocation stays ours.
            *((data_ptr as *mut isize).sub(1)) = 0;
            let constant = RocStr {
                words: [data_ptr as usize, 64 << 1, 64],
            };
            drop(constant);
            // Freeing here is the only free: had the drop released it, this
            // dealloc would trap inside the allocator.
            roc_dealloc(data_ptr, 4);
        }
    }

    /// The allocator Roc's runtime calls for every value it creates: the size
    /// header must survive realloc and the layout must match on free (a
    /// mismatch shows up as a panic inside the allocator, which is how the
    /// string-drop bug in this file was first seen).
    #[test]
    fn test_roc_allocator_round_trip() {
        unsafe {
            for (size, alignment) in [(1usize, 1u32), (8, 4), (64, 8), (4096, 16)] {
                let ptr = roc_alloc(size, alignment);
                assert!(!ptr.is_null());
                let align = (alignment as usize)
                    .max(mem::align_of::<usize>())
                    .max(ALLOC_HEADER_SIZE);
                assert_eq!(ptr as usize % align, 0, "allocation must be aligned");
                let total = *(ptr.sub(ALLOC_HEADER_SIZE) as *const usize);
                assert!(total >= ALLOC_HEADER_SIZE + size);
                ptr::write_bytes(ptr, 0xab, size);

                let bigger = roc_realloc(ptr, size * 2, alignment);
                assert!(!bigger.is_null());
                // The old bytes survive the move and the header follows.
                assert_eq!(*bigger, 0xab);
                assert_eq!(*(bigger.add(size - 1)), 0xab);
                let total = *(bigger.sub(ALLOC_HEADER_SIZE) as *const usize);
                assert!(total >= ALLOC_HEADER_SIZE + size * 2);
                roc_dealloc(bigger, alignment);
            }
        }
    }

    /// `Try`'s discriminant byte and payload offsets are what the host's calls
    /// into Roc depend on. `tooling/glue/abi_facts.roc` reports the 32-bit
    /// shape (3 words per `Str`, 16 bytes per `Try(Str, Str)`, discriminant at
    /// offset 12, `Ok = 1`); the same ratios hold at 64 bits, where the unit
    /// tests run.
    #[test]
    fn test_roc_result_layout() {
        let word = mem::size_of::<usize>();
        assert_eq!(mem::size_of::<RocStr>(), 3 * word);
        assert_eq!(mem::size_of::<RocResult<RocStr, RocStr>>(), 4 * word);

        let ok: RocResult<RocStr, RocStr> = RocResult::ok(RocStr::from_str("ok"));
        let bytes = unsafe {
            slice::from_raw_parts(&ok as *const _ as *const u8, mem::size_of_val(&ok))
        };
        assert_eq!(bytes[3 * word], 1, "Try uses 1 for Ok");
        let err: RocResult<RocStr, RocStr> = RocResult::err(RocStr::from_str("err"));
        let bytes = unsafe {
            slice::from_raw_parts(&err as *const _ as *const u8, mem::size_of_val(&err))
        };
        assert_eq!(bytes[3 * word], 0, "Try uses 0 for Err");
    }

    #[test]
    fn test_empty_roc_str() {
        let roc_str = RocStr::empty();
        assert_eq!(roc_str.as_str(), "");
    }
}
