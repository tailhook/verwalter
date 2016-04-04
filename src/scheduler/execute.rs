use std::sync::Arc;
use std::collections::{HashMap, BTreeMap};

use lua::{ThreadStatus, Type};
use rustc_serialize::json::Json;

use shared::{Id, Peer};
use config::Config;
use scheduler::{Scheduler, Error, Schedule};
use scheduler::input::Input;


impl Scheduler {
    pub fn execute(&mut self, config: &Config, peers: &HashMap<Id, Peer>,
        parents: &Vec<Arc<Schedule>>,
        actions: &BTreeMap<u64, Arc<Json>>)
        -> Result<Json, Error>
    {
        match self.lua.get_global("scheduler") {
            Type::Function => {}
            typ => {
                // TODO(tailhook) should we pop stack? Or pop only if not None
                return Err(Error::FunctionNotFound("scheduler", typ));
            }
        }
        self.lua.push(Input {
            machine: &config.machine,
            roles: &config.roles,
            peers: peers,
            id: &self.id,
            hostname: &self.hostname,
            parents: parents,
            actions: actions,
        });
        match self.lua.pcall(1, 1, 0) {
            ThreadStatus::Ok => {}
            ThreadStatus::Yield => return Err(Error::UnexpectedYield),
            err => {
                return Err(Error::Lua(err,
                    self.lua.to_str(-1).unwrap_or("undefined").to_string()))
            }
        }
        let top = self.lua.get_top();
        match self.lua.to_type::<String>(top) {
            Some(ref x) => Json::from_str(x).map_err(|_| Error::Conversion),
            None => Err(Error::Conversion),
        }
    }
}
