use std::path::{Path, PathBuf};

use lua::{State as Lua, ThreadStatus, Type, Library};
use lua::ffi::{lua_upvalueindex};
use shared::Id;

mod input;
mod main;
mod lua_json;
mod state;
mod prefetch;
mod execute;

pub use self::state::{Schedule, State, LeaderState, FollowerState, from_json};
pub use self::prefetch::PrefetchInfo;
pub use self::main::{main as run, Settings};

pub type Hash = String;

/// A number of milliseconds we are allowed to do prefeching of old data.
///
/// On the one hand, this value may be arbitrarily long, because if all data
/// is fetched leader will do its job anyway.
///
/// On the flip side, leader election doesn't happen in perfectly working
/// network. So some network issues are assumed when we have just elected.
/// And it means there is quite a high chance that some data was just lost,
/// and we will wait for this timeout.
pub const MAX_PREFETCH_TIME: i64 = 10000;

pub struct Scheduler {
    id: Id,
    hostname: String, // Is it the right place?
    lua: Lua,
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
            description("Scheduler returned unconvertible value")
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

fn load_package(lua: &mut Lua, dir: &Path) {
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
}

pub fn read(id: Id, hostname: String, base_dir: &Path)
    -> Result<Scheduler, ReadError>
{
    let dir = &base_dir.join("scheduler/v1");
    let mut lua = Lua::new();

    load_package(&mut lua, &dir);
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
    })
}
