#[macro_use] extern crate serde_json;

use std::mem;
use std::slice;
use std::panic::set_hook;
use std::os::raw::{c_void};

use serde_json::{Value, from_slice, to_vec};

extern {
    fn log_panic(payload_ptr: *const u8, payload_len: usize,
                 file_ptr: *const u8, file_len: usize, line: u32);
}

fn main() {
    set_hook(Box::new(|panic_info| {
        let payload = panic_info.payload();
        let (ptr, len) = if let Some(s) = payload.downcast_ref::<&str>() {
            (s.as_bytes().as_ptr(), s.len())
        } else if let Some(s) = payload.downcast_ref::<String>() {
            (s.as_bytes().as_ptr(), s.len())
        } else {
            (0 as *const u8, 0)
        };
        let (file_ptr, file_len, line) = match panic_info.location() {
            Some(loc) => {
                let file = loc.file().as_bytes();
                (file.as_ptr(), file.len(), loc.line())
            }
            None => (0 as *const u8, 0, 0),
        };
        unsafe {
            log_panic(ptr, len, file_ptr, file_len, line);
        }
    }));
}

#[no_mangle]
pub extern "C" fn init(ptr: *const u8, len: usize) -> *mut c_void {
    let input = unsafe { slice::from_raw_parts(ptr, len) };
    let mut out = _init_wrapper(input);
    let out_ptr = out.as_mut_ptr();
    mem::forget(out);
    return out_ptr as *mut c_void;
}

fn _init_wrapper(data: &[u8]) -> Vec<u8> {
    let input = match from_slice(data) {
        Ok(inp) => inp,
        Err(e) => {
            return to_vec(&json!({
                "Err": format!("Error deserializing input: {}", e),
            })).expect("should serialize standard json");
        }
    };
    let result = _init_inner(input);
    match to_vec(&result) {
        Ok(result) => result,
        Err(e) => {
            // TODO(pc) log error
            return to_vec(&json!({
                "Err": format!("Error serializing output: {}", e),
            })).expect("should serialize standard json");
        }
    }
}

fn _init_inner(_input: Value) -> Result<Value, String> {
    return Ok(json!(null))
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
