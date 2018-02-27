use std::collections::{VecDeque, HashMap};

use failure;
use lua::{ThreadStatus, Type, Serde, GcOption};
use serde::Serialize;
use serde_json::{from_str};

use scheduler::luatic::Scheduler;
use scheduler::main::SchedulerResult;

#[derive(Debug, Fail)]
pub enum Error {
    #[fail(display="running lua script {:?}: {}", _0, _1)]
    Lua(ThreadStatus, String),
    #[fail(display="Main expected to export {:?} expected {:?} found", _0, _1)]
    FunctionNotFound(&'static str, Type),
    #[fail(display="scheduler function should not yield")]
    UnexpectedYield,
    #[fail(display="Scheduler returned unconvertible value")]
    Conversion,
    #[fail(display="Scheduler returned nil")]
    Nil,
}

impl Scheduler {
    pub fn execute<S: Serialize>(&mut self, input: &S)
        -> Result<SchedulerResult, failure::Error>
    {
        self.lua.get_global("debug");
        self.lua.get_field(-1, "traceback");
        let error_handler = self.lua.get_top();

        self.lua.get_global("_VERWALTER_MAIN");
        match self.lua.get_field(-1, "scheduler") {
            Type::Function => {}
            typ => {
                // TODO(tailhook) should we pop stack? Or pop only if not None
                return Err(Error::FunctionNotFound("scheduler", typ).into());
            }
        }
        self.lua.push(Serde(input));
        match self.lua.pcall(1, 2, error_handler) {
            ThreadStatus::Ok => {}
            ThreadStatus::Yield => {
                return Err(Error::UnexpectedYield.into());
            }
            err => {
                let txt = self.lua.to_str(-1).unwrap_or("undefined")
                          .to_string();
                return Err(Error::Lua(err, txt).into());
            }
        }
        let top = self.lua.get_top();
        let dbg = match self.lua.to_type::<String>(top) {
            Some(x) => x,
            None => return Err(Error::Conversion.into()),
        };
        let result = match self.lua.to_type::<String>(top-1) {
            Some(ref x) => from_str(x).map_err(|_| Error::Conversion.into()),
            None => {
                if self.lua.is_nil(top-1) {
                    Err(Error::Nil.into())
                } else {
                    Err(Error::Conversion.into())
                }
            }
        };
        self.lua.pop(7);
        if result.is_err() {
            let mut vec = VecDeque::with_capacity(100);
            for line in dbg.lines() {
                if vec.len() > 100 {
                    vec.pop_front();
                }
                vec.push_back(line);
            }
            let lines = Vec::from(vec).join("\n  lua: ");
            warn!("Scheduler debug info (max 100 lines): {}", lines);
        }
        // We execute GC after every scheduler run, we are going to
        // sleep for quite a long time now, so don't care performance
        debug!("Garbage before collection: {}Kb, stack top: {}",
            self.lua.gc(GcOption::Count, 0), self.lua.get_top());
        self.lua.gc(GcOption::Collect, 0);
        info!("Garbage after collection: {}Kb",
            self.lua.gc(GcOption::Count, 0));
        result.map(|schedule| {
            SchedulerResult {
                schedule,
                log: dbg,
                actions: HashMap::new(),
            }
        })
    }
}
