use std::path::{Path, PathBuf};
use std::collections::HashMap;

use rustc_serialize::json::{Json};
use lua::{State as Lua, ThreadStatus, Type, Library};
use lua::ffi::{lua_upvalueindex};
use self::input::Input;
use config::Config;
use shared::{Id, Peer};

mod input;
mod main;
mod lua_json;
mod state;
mod prefetch;

pub use self::state::{Schedule, State, LeaderState, FollowerState};
pub use self::prefetch::PrefetchInfo;
pub use self::main::{main as run, Settings};

pub struct Scheduler {
    id: Id,
    hostname: String, // Is it the right place?
    lua: Lua,
    previous_schedule_hash: Option<String>,
}

quick_error! {
    #[derive(Debug)]
    pub enum ReadError {
        Lua(err: ThreadStatus, msg: String, path: PathBuf) {
            display("error parsing lua script {:?}: {:?}: {}", path, err, msg)
            description("error parsing lua script")
        }
        UnexpectedYield(path: PathBuf) {
            description("script loading should not yield")
        }
    }
}

quick_error! {
    #[derive(Debug)]
    pub enum Error {
        Lua(err: ThreadStatus, msg: String) {
            display("running lua script {:?}: {}", err, msg)
            description("error running scheduler")
        }
        FunctionNotFound(name: &'static str, typ: Type) {
            display("Global function {:?} expected {:?} found", name, typ)
            description("scheduler function not found")
        }
        /*
        WrongValue(val: AnyLuaValue) {
            display("script returned non-string value: {:?}", val)
        }
        */
        UnexpectedYield {
            description("scheduler function should not yield")
        }
        Conversion {
            description("Scheduler returned unconverible value")
        }
    }
}

fn lua_load_file(lua: &mut Lua) -> i32 {
    let mut path = match lua.to_str(lua_upvalueindex(1)) {
        Some(s) => PathBuf::from(s),
        None => {
            error!("Something wrong with upvalue 1");
            return 0;
        }
    };
    match lua.to_str(1) {
        Some(ref s) => path.push(s),
        None => {
            error!("No module name. (`require` without arguments?)");
            return 0;
        }
    }
    path.set_extension("lua");
    debug!("Loading {:?}", path);
    let result = lua.load_file(&format!("{}", path.display()));
    if result.is_err() {
        error!("Error loading file: {}",
            lua.to_str(-1).unwrap_or("unknown error"));
        return result as i32;
    }
    return 1;
}

pub fn read(id: Id, hostname: String, base_dir: &Path)
    -> Result<Scheduler, ReadError>
{
    let dir = &base_dir.join("scheduler/v1");
    let mut lua = Lua::new();
    let tbl = lua.open_package();
    lua.get_field(tbl, "searchers");
    let srch = lua.get_top();
    lua.push_integer(1);
    lua.push_string(&format!("{}", dir.display()));
    lua.push_closure(lua_func!(lua_load_file), 1);
    lua.set_table(srch);
    lua.push_integer(2);
    lua.push_nil();
    lua.set_table(srch);
    lua.push_integer(3);
    lua.push_nil();
    lua.set_table(srch);
    lua.push_integer(4);
    lua.push_nil();
    lua.set_table(srch);
    lua.pop(1);

    //lua.load_library(Library::Package);
    lua.load_library(Library::Base);
    lua.load_library(Library::Table);
    lua.load_library(Library::String);
    lua.load_library(Library::Utf8);
    lua.load_library(Library::Math);

    let path = dir.join("main.lua");
    {
        try!(match lua.do_file(&path.to_str().unwrap_or("undecodable")) {
            ThreadStatus::Ok => Ok(()),
            ThreadStatus::Yield
            => Err(ReadError::UnexpectedYield(path.clone())),
            x => {
                let e = lua.to_str(-1).unwrap_or("undefined").to_string();
                Err(ReadError::Lua(x, e, path.clone()))
            }
        });
    }
    debug!("Scheduler loaded");
    Ok(Scheduler {
        id: id,
        hostname: hostname,
        lua: lua,
        previous_schedule_hash: None,
    })
}

impl Scheduler {
    pub fn execute(&mut self, config: &Config, peers: &HashMap<Id, Peer>)
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
