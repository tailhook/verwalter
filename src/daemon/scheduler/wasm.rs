use std::path::{Path};

use failure::Error;
use serde::Serialize;

use scheduler::main::SchedulerResult;
use wasm::Program;

pub(in scheduler) struct Scheduler {
    wasm: Program,
}

impl Scheduler {
    pub fn read(dir: &Path)
        -> Result<Scheduler, Error>
    {
        Program::read(&dir.join("scheduler.wasm"))
        .map(|wasm| Scheduler { wasm })
    }
    pub fn execute<S: Serialize>(&mut self, input: &S)
        -> Result<SchedulerResult, Error>
    {
        self.wasm.json_call("scheduler", input)
    }
}
