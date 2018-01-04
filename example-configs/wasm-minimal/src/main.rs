#[macro_use] extern crate serde_json;

use std::mem;
use std::slice;
use std::fmt::Write;
use std::os::raw::{c_void};

use serde_json::{Value, from_slice, to_vec};


fn main() {
}

#[no_mangle]
pub extern "C" fn scheduler(ptr: *const u8, len: usize) -> *mut c_void {
    let input = unsafe { slice::from_raw_parts(ptr, len) };
    let mut out = _scheduler_wrapper(input);
    let out_ptr = out.as_mut_ptr();
    mem::forget(out);
    return out_ptr as *mut c_void;
}

fn _scheduler_wrapper(data: &[u8]) -> Vec<u8> {
    let input = match from_slice(data) {
        Ok(inp) => inp,
        Err(e) => {
            return to_vec(
                &json!([{}, format!("Error deserialing input: {}", e)])
            ).expect("can serialize error")
        }
    };
    let result = _scheduler_inner(input);
    match to_vec(&result) {
        Ok(result) => result,
        Err(e) => {
            let (_, mut debug) = result;
            writeln!(&mut debug, "\nError serializing input: {}", e).ok();
            return to_vec(&json!([{}, debug])).expect("can serialize error")
        }
    }
}

fn _scheduler_inner(_input: Value) -> (Value, String) {
    return (json!({}), "Scheduler works!".into())
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
