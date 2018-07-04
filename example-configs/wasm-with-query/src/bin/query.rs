extern crate serde;
#[macro_use] extern crate serde_derive;
#[macro_use] extern crate serde_json;

use std::mem;
use std::slice;
use std::panic::set_hook;
use std::os::raw::{c_void};

use serde::{Serialize, Deserialize};
use serde_json::{Value, from_slice, to_vec};

extern {
    fn log_panic(payload_ptr: *const u8, payload_len: usize,
                 file_ptr: *const u8, file_len: usize, line: u32);
}

#[derive(Debug, Serialize)]
enum ErrorKind {
    Serialize,
    Deserialize,
    Internal,
}

#[derive(Debug, Serialize)]
struct QueryError {
    kind: ErrorKind,
    message: String,
    causes: Option<Vec<String>>,
    backtrace: Option<String>,
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
    let mut out = _wrapper(input, _init);
    let out_ptr = out.as_mut_ptr();
    mem::forget(out);
    return out_ptr as *mut c_void;
}

#[no_mangle]
pub extern "C" fn render_roles(ptr: *const u8, len: usize) -> *mut c_void {
    let input = unsafe { slice::from_raw_parts(ptr, len) };
    let mut out = _wrapper(input, _render_roles);
    let out_ptr = out.as_mut_ptr();
    mem::forget(out);
    return out_ptr as *mut c_void;
}

fn _render_roles(_input: Value) -> Result<Value, String> {
    return Ok(json!({"imaginary_role": {}}))
}

fn _wrapper<'x, F, S, D>(data: &'x [u8], f: F) -> Vec<u8>
    where
        F: Fn(D) -> Result<S, String>,
        D: Deserialize<'x>,
        S: Serialize
{
    let input = match from_slice(data) {
        Ok(inp) => inp,
        Err(e) => {
            return to_vec(
                &Err::<(), _>(QueryError {
                    kind: ErrorKind::Deserialize,
                    message: e.to_string(),
                    causes: None,
                    backtrace: None,
                })
            ).expect("should serialize standard json");
        }
    };
    let result = f(input);
    match to_vec(&result) {
        Ok(result) => result,
        Err(e) => {
            return to_vec(
                &Err::<(), _>(QueryError {
                    kind: ErrorKind::Serialize,
                    message: e.to_string(),
                    causes: None,
                    backtrace: None,
                })
            ).expect("should serialize standard json");
        }
    }
}

fn _init(_input: Value) -> Result<Value, String> {
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
