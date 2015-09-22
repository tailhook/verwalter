use std::fs::File;
use std::path::{Path, PathBuf};

use rustc_serialize::json::{Json, BuilderError};
use hlua::{Lua, LuaError};
use hlua::any::{AnyLuaValue};


pub struct Scheduler<'lua> {
    lua: Lua<'lua>,
}

quick_error! {
    #[derive(Debug)]
    pub enum ReadError {
        Lua(err: LuaError, path: PathBuf) {
            display("error reading lua script {:?}: {:?}", path, err)
        }
    }
}

quick_error! {
    #[derive(Debug)]
    pub enum Error {
        Lua(err: LuaError) {
            from()
            display("running lua script: {:?}", err)
        }
        WrongValue(val: AnyLuaValue) {
            display("script returned non-string value: {:?}", val)
        }
        Json(err: BuilderError) {
            from() cause(err)
            display("script returned bad json: {:?}", err)
        }
    }
}

pub fn read(base_dir: &Path) -> Result<Scheduler, ReadError> {
    let mut lua = Lua::new();

    // TODO(tailhook) remove me!!!
    // should have more control over which libraries to use
    lua.openlibs();

    let path = &base_dir.join("scheduler/main.lua");
    {
        let f = try!(File::open(&path)
            .map_err(|e| ReadError::Lua(
                LuaError::ReadError(e), path.clone())));
        try!(lua.execute_from_reader(f)
            .map_err(|e| ReadError::Lua(e, path.clone())));
    }
    Ok(Scheduler {
        lua: lua,
    })
}

impl<'lua> Scheduler<'lua> {
    pub fn execute(&mut self) -> Result<Json, Error> {
        // TODO(tailhook) this shouldn't set global state but rather
        // push a variable on the stack and call a function
        {
            let mut tbl = self.lua.empty_array("state");
            tbl.set("verwalter_version",
                concat!("v", env!("CARGO_PKG_VERSION")));
        }
        let val = try!(self.lua.execute::<AnyLuaValue>(
            "return scheduler(state)"));
        match val {
            AnyLuaValue::LuaString(x) => Ok(try!(Json::from_str(&x[..]))),
            val => Err(Error::WrongValue(val)),
        }
    }
}
