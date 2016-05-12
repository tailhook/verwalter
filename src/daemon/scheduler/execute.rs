use std::sync::Arc;
use std::collections::{HashMap, BTreeMap};

use lua::{ThreadStatus, Type};
use rotor_cantal::RemoteQuery;
use rustc_serialize::json::Json;

use shared::{Id, Peer};
use config::Runtime;
use scheduler::{Scheduler, Error, Schedule};
use scheduler::input::Input;


impl Scheduler {
    pub fn execute(&mut self, runtime: &Runtime, peers: &HashMap<Id, Peer>,
        parents: &Vec<Arc<Schedule>>,
        actions: &BTreeMap<u64, Arc<Json>>,
        metrics: Option<Arc<RemoteQuery>>)
        -> (Result<Json, Error>, String)
    {
        match self.lua.get_global("scheduler") {
            Type::Function => {}
            typ => {
                // TODO(tailhook) should we pop stack? Or pop only if not None
                return (Err(Error::FunctionNotFound("scheduler", typ)),
                    String::from("Scheduler function not found"));
            }
        }
        self.lua.push(Input {
            runtime: runtime,
            peers: peers,
            id: &self.id,
            hostname: &self.hostname,
            parents: parents,
            actions: actions,
            metrics: metrics,
        });
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
        self.lua.pop(top);
        self.lua.pop(top-1);
        return (result, dbg);
    }
}
