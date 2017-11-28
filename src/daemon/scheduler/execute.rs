use lua::{ThreadStatus, Type, Serde};
use serde::Serialize;
use serde_json::{Value as Json, from_str};
use std::collections::VecDeque;

use scheduler::{Scheduler, Error};


impl Scheduler {
    pub fn execute<S: Serialize>(&mut self, input: &S)
        -> (Result<Json, Error>, String)
    {
        self.lua.get_global("debug");
        self.lua.get_field(-1, "traceback");
        let error_handler = self.lua.get_top();

        self.lua.get_global("_VERWALTER_MAIN");
        match self.lua.get_field(-1, "scheduler") {
            Type::Function => {}
            typ => {
                // TODO(tailhook) should we pop stack? Or pop only if not None
                return (Err(Error::FunctionNotFound("scheduler", typ)),
                    String::from("Scheduler function not found"));
            }
        }
        self.lua.push(Serde(input));
        match self.lua.pcall(1, 2, error_handler) {
            ThreadStatus::Ok => {}
            ThreadStatus::Yield => {
                return (Err(Error::UnexpectedYield),
                    String::from("Scheduler yielded instead of returning"));
            }
            err => {
                let txt = self.lua.to_str(-1).unwrap_or("undefined")
                          .to_string();
                let dbg = format!("Lua call failed: {}", txt);
                return (Err(Error::Lua(err, txt)), dbg);
            }
        }
        let top = self.lua.get_top();
        let dbg = match self.lua.to_type::<String>(top) {
            Some(x) => x,
            None => return (Err(Error::Conversion),
                            String::from("Debug info is of wrong type")),
        };
        let result = match self.lua.to_type::<String>(top-1) {
            Some(ref x) => from_str(x).map_err(|_| Error::Conversion),
            None => Err(Error::Conversion),
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
        return (result, dbg);
    }
}
