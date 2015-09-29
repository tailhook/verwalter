use lua::{ToLua, State};

use config::Config;
use super::lua_json::push_json;


impl<'a> ToLua for &'a Config {
    fn to_lua(&self, lua: &mut State) {
        lua.new_table(); // Config
        let cfg = lua.get_top();
        match &self.machine {
            &Ok(ref metadata) => {
                push_json(lua, &metadata);
                lua.set_field(cfg, "machine");
            }
            &Err(ref e) => {
                push_json(lua, &e.partial);
                lua.set_field(cfg, "machine_partial");
                lua.push_integer(e.errors.len() as i64);
                lua.set_field(cfg, "machine_error_num");
            }
        }
        lua.new_table(); // roles
        let roles = lua.get_top();
        for (name, role) in self.roles.iter() {
            lua.new_table(); // role
            let role_idx = lua.get_top();
            lua.new_table(); // runtime
            let runtime_idx = lua.get_top();
            for (ver, runtime) in role.runtime.iter() {
                match runtime {
                    &Ok(ref metadata) => {
                        push_json(lua, metadata);
                        lua.set_field(runtime_idx, &ver.0);
                    }
                    &Err(_)  => {}
                }
            }
            lua.set_field(role_idx, "runtime");
            lua.new_table(); // runtime
            let runtime_err_idx = lua.get_top();
            for (ver, runtime) in role.runtime.iter() {
                match runtime {
                    &Ok(_) => {}
                    &Err(ref e)  => {
                        lua.new_table(); // error
                        let err_table = lua.get_top();
                        push_json(lua, &e.partial);
                        lua.set_field(err_table, "partial");
                        lua.push_integer(e.errors.len() as i64);
                        lua.set_field(err_table, "error_num");
                        lua.set_field(runtime_err_idx, &ver.0);
                    }
                }
            }
            lua.set_field(role_idx, "runtime_errors");
            lua.set_field(roles, &name); // role
        }
        lua.set_field(cfg, "roles");
    }
}
