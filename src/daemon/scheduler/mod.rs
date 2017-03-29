use std::path::{Path, PathBuf};

use lua::{State as Lua, ThreadStatus, Type, Library};
use lua::ffi::{lua_upvalueindex};

use id::Id;

mod state;
mod prefetch;
/*
pub mod main;  // pub for making counters visible
mod lua_json;
mod execute;
*/

pub use self::state::{Schedule, State, LeaderState, FollowerState, from_json};
/*
pub use self::prefetch::PrefetchInfo;
pub use self::main::{main as run, Settings};
*/


pub type Hash = String;

/*
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
            display("lua script error {:?} -> {:?}: {}", path, err, msg)
            description("error parsing lua script")
        }
        File(err: ThreadStatus, msg: String, path: PathBuf) {
            display("cound not read file {:?} ->  {:?}: {}", path, err, msg)
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
            display("Main expected to export {:?} expected {:?} found",
                name, typ)
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
        let err_idx = lua.get_top();
        lua.get_global("_VERWALTER_ERRORS");
        let tbl_idx = lua.get_top();
        let tbl_len = lua.raw_len(tbl_idx);
        // This returns text, but also pushes string on the stack
        {
            let err_text = lua.to_str(err_idx).unwrap_or("unknown error");
            error!("Error loading file: {}", err_text);
        }
        lua.raw_seti(tbl_idx, (tbl_len+1) as i64);
        lua.pop(1); // table
        assert_eq!(err_idx, lua.get_top());
        return 0;
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
    // TODO(tailhook) include debug.traceback but not interactive functions
    lua.load_library(Library::Debug);

    lua.get_global("debug");
    lua.get_field(-1, "traceback");
    let error_handler = lua.get_top();

    lua.new_table();
    lua.set_global("_VERWALTER_ERRORS");

    let path = dir.join("main.lua");
    let result = {
        let strpath = match path.to_str() {
            Some(x) => x,
            None => return Err(ReadError::File(ThreadStatus::FileError,
                "can't stringify path".into(), path.to_path_buf())),
        };
        match lua.load_file(&strpath) {
            ThreadStatus::Ok => {},
            err @ ThreadStatus::Yield => {
                return Err(ReadError::File(err,
                    "unexpected yield".into(), path.to_path_buf()));
            }
            err => {
                let msg = lua.to_str_in_place(-1)
                    .unwrap_or("no message").to_string();
                lua.pop(-1);
                return Err(ReadError::File(err, msg, path.to_path_buf()));
            }
        }
        // TODO(tailhook) pass error function
        match lua.pcall(0, 1, error_handler) {
            ThreadStatus::Ok => {
                lua.set_global("_VERWALTER_MAIN");
                Ok(())
            }
            err @ ThreadStatus::Yield => {
                return Err(ReadError::File(err,
                    "unexpected yield".to_string(), path.to_path_buf()));
            }
            x => {
                let mut e = lua.to_str_in_place(-1)
                    .unwrap_or("unconvertible_error").to_string();
                lua.get_global("_VERWALTER_ERRORS");
                let err_idx = lua.get_top();
                let err_num = lua.raw_len(err_idx) as i64;
                for i in (1..err_num+1).rev() {
                    lua.raw_geti(err_idx, i);
                    e = format!("{}\n{}",
                        lua.to_str_in_place(-1)
                            .unwrap_or("unconvertible_place"),
                        e);
                    lua.pop(-1);
                }
                lua.pop(-1);
                Err(ReadError::Lua(x, e, path.clone()))
            }
        }
    };

    lua.push_nil();
    lua.set_global("_VERWALTER_ERRORS");

    lua.pop(2); // error handler, and global

    result.map(|()| Scheduler {
        id: id,
        hostname: hostname,
        lua: lua,
    })
}
*/
