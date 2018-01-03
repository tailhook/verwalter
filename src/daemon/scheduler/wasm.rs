use std::io;
use std::path::{Path};
use std::sync::Arc;

use failure::{Error, err_msg};
use serde_json::{Value as Json, to_vec, from_reader};
use serde::Serialize;
use parity_wasm::{ProgramInstance, deserialize_file, ModuleInstance};
use parity_wasm::{ModuleInstanceInterface};
use parity_wasm::RuntimeValue::I32;
use parity_wasm::interpreter::{ItemIndex, MemoryInstance};


pub(in scheduler) struct Scheduler {
    module: Arc<ModuleInstance>,
    memory: Arc<MemoryInstance>,
}

struct ReadMemory {
    memory: Arc<MemoryInstance>,
    offset: usize,
}

pub(in scheduler) fn read(dir: &Path)
    -> Result<Scheduler, Error>
{
    let program = ProgramInstance::new();

    // Here we load module using dedicated for this purpose
    // `deserialize_file` function (which works only with modules)
    let module = deserialize_file(dir.join("scheduler.wasm"))
        .map_err(|e| err_msg(format!("error decoding wasm: {:?}", e)))?;
    let module = program.add_module("main", module, None)
        .map_err(|e| err_msg(format!("error adding wasm module: {}", e)))?;
    let memory = module.memory(ItemIndex::Internal(0))
        .map_err(|e| err_msg(format!("wasm memory error: {}", e)))?;
    Ok(Scheduler { module, memory })
}

impl Scheduler {
    pub fn execute<S: Serialize>(&mut self, input: &S)
        -> (Result<Json, Error>, String)
    {
        let bytes = match to_vec(input) {
            Ok(bytes) => bytes,
            Err(e) => return (Err(e.into()),
                              String::from("failed to encode input")),
        };
        let off = match
            self.module.execute_export("alloc",
                vec![I32(bytes.len() as i32)].into())
        {
            Ok(Some(I32(off))) => off as u32,
            Ok(x) => return (
                Err(err_msg(format!("alloc invalid: {:?}", x))),
                String::from("failed to allocate memory")),
            Err(e) => return (
                Err(err_msg(format!("failed to allocate memory: {}", e))),
                String::from("failed to allocate memory")),
        };
        match self.memory.set(off, &bytes) {
            Ok(()) => {},
            Err(e) => return (
                Err(err_msg(format!("failed to write memory: {}", e))),
                String::from("failed to write memory")),
        };
        let roff = self.module.execute_export("scheduler",
            vec![I32(off as i32), I32(bytes.len() as i32)].into());
        match self.module.execute_export("dealloc", vec![I32(off as i32)].into()) {
            Ok(_) => {}
            Err(e) => return (
                Err(err_msg(format!("failed to deallocate memory: {}", e))),
                String::from("failed to deallocate memory")),
        };
        let roff = match roff {
            Ok(Some(I32(off))) => off as u32,
            Ok(x) => return (
                Err(err_msg(format!("bad scheduler result: {:?}", x))),
                String::from("scheduler result error")),
            Err(e) => return (
                Err(err_msg(format!("bad scheduler result: {}", e))),
                String::from("scheduler result error")),
        };
        let reader = ReadMemory {
            memory: self.memory.clone(),
            offset: off as usize,
        };
        let (result, debug) = match from_reader(reader) {
            Ok((result, debug)) => (result, debug),
            Err(e) => return (
                Err(err_msg(format!("failed to \
                    deserialize scheduler result: {}", e))),
                String::from("deserialize error")),
        };
        match self.module.execute_export("dealloc", vec![I32(roff as i32)].into()) {
            Ok(_) => {}
            Err(e) => return (
                Err(err_msg(format!("failed to deallocate memory: {}", e))),
                String::from("failed to deallocate memory")),
        };
        (Ok(result), debug)
    }
}

impl io::Read for ReadMemory {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        let ref mut off = self.offset;
        let dest = if *off + buf.len() > self.memory.size() as usize {
            buf.split_at_mut(self.memory.size() as usize - *off).0
        } else {
            buf
        };
        match self.memory.get_into(*off as u32, dest) {
            Ok(()) => {
                *off += dest.len();
                Ok(dest.len())
            }
            Err(e) => Err(io::Error::new(io::ErrorKind::Other, e.to_string())),
        }
    }
}
