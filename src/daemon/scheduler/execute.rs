use lua::{ThreadStatus, Type};
use rustc_serialize::json::Json;

use scheduler::{Scheduler, Error};
use scheduler::lua_json::push_json;


impl Scheduler {
    pub fn execute(&mut self, input: &Json)
        -> (Result<Json, Error>, String)
    {
        self.lua.get_global("_VERWALTER_MAIN");
        match self.lua.get_field(-1, "scheduler") {
            Type::Function => {}
            typ => {
                // TODO(tailhook) should we pop stack? Or pop only if not None
                return (Err(Error::FunctionNotFound("scheduler", typ)),
                    String::from("Scheduler function not found"));
            }
        }
        push_json(&mut self.lua, input);
        match self.lua.pcall(1, 2, 0) {
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
            Some(ref x) => Json::from_str(x).map_err(|_| Error::Conversion),
            None => Err(Error::Conversion),
        };
        self.lua.pop(5);
        return (result, dbg);
    }
}
