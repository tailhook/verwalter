use std::mem;
use std::os::raw::{c_void};


fn main() {
}

// In order to work with the memory we expose (de)allocation methods
#[no_mangle]
pub extern "C" fn alloc(size: usize) -> *mut c_void {
    let mut buf = Vec::with_capacity(size);
    let ptr = buf.as_mut_ptr();
    mem::forget(buf);
    return ptr as *mut c_void;
}

#[no_mangle]
pub extern "C" fn dealloc(ptr: *mut c_void) {
    unsafe  {
        let _buf = Vec::from_raw_parts(ptr, 0, 1);
    }
}
