use std::path::{Path, PathBuf};

use rustc_serialize::json::{Json};
use lua::{State, ThreadStatus, Type};
use config::Config;

mod config;
mod lua_json;

pub struct Scheduler {
    lua: State,
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

pub fn read(base_dir: &Path) -> Result<Scheduler, ReadError> {
    let mut lua = State::new();

    // TODO(tailhook) remove me!!!
    // should have more control over which libraries to use
    lua.open_libs();

    let path = &base_dir.join("scheduler/v1/main.lua");
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
    Ok(Scheduler {
        lua: lua,
    })
}

impl Scheduler {
    pub fn execute(&mut self, config: &Config) -> Result<Json, Error> {
        match self.lua.get_global("scheduler") {
            Type::Function => {}
            typ => {
                // TODO(tailhook) should we pop stack? Or pop only if not None
                return Err(Error::FunctionNotFound("scheduler", typ));
            }
        }
        self.lua.push(config);
        match self.lua.pcall(1, 1, 0) {
            ThreadStatus::Ok => {}
            ThreadStatus::Yield => return Err(Error::UnexpectedYield),
            err => {
                return Err(Error::Lua(err,
                    self.lua.to_str(-1).unwrap_or("undefined").to_string()))
            }
        }
        match self.lua.to_type::<String>() {
            Some(ref x) => Json::from_str(x).map_err(|_| Error::Conversion),
            None => Err(Error::Conversion),
        }
    }
}
