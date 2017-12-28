use std::path::Path;

use failure::Error;
use serde_json::{Value as Json};

mod execute;
mod state;
pub mod main;  // pub for making counters visible
mod luatic;
mod wasm;

pub use self::state::{Schedule, ScheduleId, from_json};
pub use self::main::{main as run, Settings, SchedulerInput};

enum Scheduler {
    Lua(self::luatic::Scheduler),
    Wasm(self::wasm::Scheduler),
}

impl Scheduler {
    fn execute(&mut self, input: &SchedulerInput)
        -> (Result<Json, Error>, String)
    {
        use self::Scheduler::*;
        match *self {
            Lua(ref mut scheduler) => scheduler.execute(&input),
            Wasm(ref mut scheduler) => scheduler.execute(&input),
        }
    }
}

pub(in scheduler) fn read(base_dir: &Path)
    -> Result<Scheduler, Error>
{
    let ref dir = &base_dir.join("scheduler/v1");
    if dir.join("scheduler.wasm").exists() {
        Ok(Scheduler::Wasm(self::wasm::read(dir)?))
    } else {
        Ok(Scheduler::Lua(self::luatic::read(dir)?))
    }
}
