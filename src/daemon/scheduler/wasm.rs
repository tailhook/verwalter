use std::io::{self, Read, BufReader};
use std::fs::File;
use std::path::{Path};

use failure::{Error, err_msg};
use serde_json::{Value as Json, to_vec, de};
use serde::Serialize;
use wasmi::{self, load_from_buffer, ModuleInstance, ImportsBuilder, ModuleRef};
use wasmi::RuntimeValue::I32;
use wasmi::{MemoryRef};
use wasmi::{Externals, RuntimeValue, RuntimeArgs, ModuleImportResolver};
use wasmi::{FuncRef, ValueType, Signature, FuncInstance};


const PAGE_SIZE: usize = 65536; // for some reason it's not in interpreter
const PANIC_INDEX: usize = 0;
const POW_INDEX: usize = 100;
const FMOD_INDEX: usize = 101;
const EXP2_INDEX: usize = 102;
const LDEXP_INDEX: usize = 104;
const LOG10_INDEX: usize = 106;


pub(in scheduler) struct Scheduler {
    module: ModuleRef,
    memory: MemoryRef,
    scheduler_util: Util,
}

struct ReadMemory {
    memory: MemoryRef,
    offset: usize,
}

struct Resolver;

struct Util {
    memory: MemoryRef,
}


pub(in scheduler) fn read(dir: &Path)
    -> Result<Scheduler, Error>
{
    // Here we load module using dedicated for this purpose
    // `deserialize_file` function (which works only with modules)
    let path = dir.join("scheduler.wasm");
    let mut buf = Vec::new();
    File::open(&path)
        .and_then(|mut f| f.read_to_end(&mut buf))
        .map_err(|e| err_msg(format!("Error reading {:?}: {}", path, e)))?;
    let module = load_from_buffer(&buf)
        .map_err(|e| err_msg(format!("error decoding wasm: {:?}", e)))?;
    let module = ModuleInstance::new(
            &module,
            &ImportsBuilder::new()
            .with_resolver("env", &Resolver),
        ).map_err(|e| err_msg(format!("error adding wasm module: {}", e)))?;
    let memory = module.not_started_instance().export_by_name("memory")
        .and_then(|x| x.as_memory().map(Clone::clone))
        .ok_or_else(|| err_msg("no memory exported"))?;
    let mut scheduler_util = Util { memory: memory.clone() };
    let module = module.run_start(&mut scheduler_util)
        .map_err(|e| err_msg(format!("error starting wasm module: {}", e)))?;
    Ok(Scheduler { module, memory, scheduler_util })
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
            self.module.invoke_export("alloc",
                &[I32(bytes.len() as i32)],
                &mut self.scheduler_util)
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
        let roff = self.module.invoke_export("scheduler",
            &[I32(off as i32), I32(bytes.len() as i32)],
            &mut self.scheduler_util);
        match self.module.invoke_export("dealloc",
            &[I32(off as i32)],
            &mut self.scheduler_util)
        {
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
        match self.module.invoke_export("dealloc",
            &[I32(roff as i32)], &mut self.scheduler_util)
        {
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

impl ModuleImportResolver for Resolver {
    fn resolve_func(&self, field_name: &str, signature: &Signature)
        -> Result<FuncRef, wasmi::Error>
    {
        let idx = match field_name {
            "log_panic"
            if signature == &Signature::new(&[ValueType::I32; 5][..], None)
            => PANIC_INDEX,
            "log_panic" => return Err(wasmi::Error::Instantiation(
                format!("Export {} expects invalid signature {:?}",
                    field_name, signature)
            )),
            "pow" if signature.params().len() == 2 => POW_INDEX,
            "pow" => return Err(wasmi::Error::Instantiation(
                format!("Export {} expects invalid signature {:?}",
                    field_name, signature)
            )),
            "fmod" if signature.params().len() == 2 => FMOD_INDEX,
            "fmod" => return Err(wasmi::Error::Instantiation(
                format!("Export {} expects invalid signature {:?}",
                    field_name, signature)
            )),
            "exp2"|"exp2f" if signature.params().len() == 1 => EXP2_INDEX,
            "exp2"|"exp2f" => return Err(wasmi::Error::Instantiation(
                format!("Export {} expects invalid signature {:?}",
                    field_name, signature)
            )),
            "ldexp"|"ldexpf" if signature.params().len() == 2 => LDEXP_INDEX,
            "ldexp"|"ldexpf" => return Err(wasmi::Error::Instantiation(
                format!("Export {} expects invalid signature {:?}",
                    field_name, signature)
            )),
            "log10"|"log10f" if signature.params().len() == 1 => LOG10_INDEX,
            "log10"|"log10f" => return Err(wasmi::Error::Instantiation(
                format!("Export {} expects invalid signature {:?}",
                    field_name, signature)
            )),
            _ => return Err(wasmi::Error::Instantiation(
                format!("Export {} not found", field_name),
            ))
        };
        return Ok(FuncInstance::alloc_host(signature.clone(), idx));
    }
}

impl Externals for Util {
    fn invoke_index(&mut self, index: usize, args: RuntimeArgs)
        -> Result<Option<RuntimeValue>, wasmi::Error>
    {
        use wasmi::RuntimeValue::*;
        let a1 = args.nth_value(0);
        let a2 = args.nth_value(1);
        let res = match index {
            PANIC_INDEX => {
                // panic(payload_str, payload_len, file_ptr, file_len, line);
                let payload_ptr: Result<u32, _> = args.nth(0);
                let payload_len: Result<u32, _> = args.nth(1);
                let file_ptr: Result<u32, _> = args.nth(2);
                let file_len: Result<u32, _> = args.nth(3);
                let line: Result<u32, _> = args.nth(4);
                let payload = match (payload_ptr, payload_len) {
                    (_, Ok(0)) | (_, Err(_)) | (Err(_), _) => {
                        "<non-str payload>".into()
                    }
                    (Ok(off), Ok(len)) => {
                        let mut buf = vec![0u8; len as usize];
                        match self.memory.get_into(off, &mut buf) {
                            Ok(()) => String::from_utf8_lossy(&buf).to_string(),
                            Err(e) => {
                                debug!("Error reading panic payload: {}", e);
                                "<can't read payload>".into()
                            }
                        }
                    }
                };
                let filename = match (file_ptr, file_len) {
                    (_, Ok(0)) | (_, Err(_)) | (Err(_), _) => {
                        "<unknown file>".into()
                    }
                    (Ok(off), Ok(len)) => {
                        let mut buf = vec![0u8; len as usize];
                        match self.memory.get_into(off, &mut buf) {
                            Ok(()) => String::from_utf8_lossy(&buf).to_string(),
                            Err(e) => {
                                debug!("Error reading panic filename: {}", e);
                                "<can't read filename>".into()
                            }
                        }
                    }
                };
                error!("Scheduler panicked at {:?}:{}: {:?}",
                    filename, line.unwrap_or(0), payload);
                return Ok(None)
            }
            POW_INDEX => match (a1.unwrap(), a2.unwrap()) {
                (I32(a), I32(b)) => I32(i32::pow(a, b as u32)),
                (I64(a), I32(b)) => I64(i64::pow(a, b as u32)),
                (F32(a), I32(b)) => F32(f32::powi(a, b)),
                (F64(a), I32(b)) => F64(f64::powi(a, b)),
                (a, b) => return Err(wasmi::Error::Trap(
                    format!("Invalid args for pow: {:?} / {:?}", a, b))),
            },
            FMOD_INDEX => match (a1.unwrap(), a2.unwrap()) {
                (F32(a), F32(b)) => F32(a % b),
                (F64(a), F64(b)) => F64(a % b),
                (a, b) => return Err(wasmi::Error::Trap(
                    format!("Invalid args for fmod: {:?} / {:?}", a, b))),
            },
            EXP2_INDEX => match a1.unwrap() {
                I32(a) => F64((a as f64).exp2()),
                I64(a) => F64((a as f64).exp2()),
                F32(a) => F32(a.exp2()),
                F64(a) => F64(a.exp2()),
            },
            LDEXP_INDEX => match (a1.unwrap(), a2.unwrap()) {
                (F32(a), I32(b)) => F32(a*(b as f32).exp2()),
                (F64(a), I32(b)) => F64(a*(b as f64).exp2()),
                (F32(a), F32(b)) => F32(a*b.exp2()),
                (F64(a), F64(b)) => F64(a*b.exp2()),
                (a, b) => return Err(wasmi::Error::Trap(
                    format!("Invalid args for ldexp: {:?} / {:?}", a, b))),
            },
            LOG10_INDEX => match a1.unwrap() {
                I32(a) => F64((a as f64).log10()),
                I64(a) => F64((a as f64).log10()),
                F32(a) => F32(a.log10()),
                F64(a) => F64(a.log10()),
            },
            _ => panic!("Unimplemented function at {}", index),
        };
        return Ok(Some(res));
    }
}
