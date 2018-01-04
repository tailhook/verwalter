use std::io::{self, BufReader};
use std::path::{Path};
use std::sync::Arc;

use failure::{Error, err_msg};
use serde_json::{Value as Json, to_vec, de};
use serde::Serialize;
use parity_wasm::{ProgramInstance, deserialize_file, ModuleInstance};
use parity_wasm::{ModuleInstanceInterface};
use parity_wasm::RuntimeValue::I32;
use parity_wasm::interpreter::{ItemIndex, MemoryInstance};

const PAGE_SIZE: usize = 65536; // for some reason it's not in interpreter


pub(in scheduler) struct Scheduler {
    #[allow(dead_code)] // need to keep in memory
    program: ProgramInstance,
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
    Ok(Scheduler { module, memory, program })
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
            Ok(Some(I32(off))) => {
                let memsize = self.memory.size() * PAGE_SIZE as u32;
                if off as u32 >= memsize {
                    return (
                        Err(err_msg(format!("scheduler returned offset {} \
                            but memory size is {}", off, memsize))),
                        String::from("scheduler result error"))
                }
                off as u32
            }
            Ok(x) => return (
                Err(err_msg(format!("bad scheduler result: {:?}", x))),
                String::from("scheduler result error")),
            Err(e) => return (
                Err(err_msg(format!("bad scheduler result: {}", e))),
                String::from("scheduler result error")),
        };
        let reader = ReadMemory {
            memory: self.memory.clone(),
            offset: roff as usize,
        };
        let de = de::Deserializer::from_reader(BufReader::new(reader));
        let (result, debug) = match de.into_iter().next() {
            Some(Ok((result, debug))) => (result, debug),
            None => return (
                Err(err_msg(format!("failed to \
                    deserialize scheduler result: no value decoded"))),
                String::from("deserialize error")),
            Some(Err(e)) => return (
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
        let memsize = self.memory.size() as usize * PAGE_SIZE;
        debug_assert!(*off <= memsize);
        let dest = if *off + buf.len() > memsize {
            buf.split_at_mut(memsize - *off).0
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
