use std::sync::Arc;
use std::collections::{HashMap, BTreeMap};

use time::get_time;
use lua::{ToLua, State};
use rustc_serialize::json::{Json};

use config::{MetadataErrors, Role};
use shared::{Id, Peer};
use time_util::ToMsec;
use super::Schedule;
use super::lua_json::{push_json, push_json_object_with_id};

pub struct Input<'a> {
    pub machine: &'a Result<Json, MetadataErrors>,
    pub roles: &'a HashMap<String, Role>,
    pub peers: &'a HashMap<Id, Peer>,
    pub hostname: &'a str,
    pub id: &'a Id,
    pub parents: &'a Vec<Arc<Schedule>>,
    pub actions: &'a BTreeMap<u64, Arc<Json>>,
}


impl<'a> ToLua for Input<'a> {
    fn to_lua(&self, lua: &mut State) {

        lua.new_table(); // Config
        let cfg = lua.get_top();

        // These make configuration non-idempotent, not sure of them
        lua.push_number(get_time().to_msec() as f64);
        lua.set_field(cfg, "now");
        lua.push_string(self.hostname);
        lua.set_field(cfg, "current_host");
        lua.push_string(&self.id.to_string());
        lua.set_field(cfg, "current_id");
        // end

        {
            lua.create_table(self.parents.len() as i32, 0);
            let tbl = lua.get_top();
            for (i, item) in self.parents.iter().enumerate() {
                push_json(lua, &item.data);
                lua.raw_seti(tbl, (i+1) as i64);
            }
            lua.set_field(cfg, "parents");
        }

        {
            lua.create_table(self.actions.len() as i32, 0);
            let tbl = lua.get_top();
            for (i, (id, value)) in self.actions.iter().enumerate() {
                push_json_object_with_id(lua, &value, *id);
                lua.raw_seti(tbl, (i+1) as i64);
            }
            lua.set_field(cfg, "actions");
        }

        match self.machine {
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

        lua.new_table(); // Peers
        let peers = lua.get_top();
        for (id, peer) in self.peers.iter() {
            lua.new_table();
            let idx = lua.get_top();

            lua.push_string(&peer.hostname);
            lua.set_field(idx, "hostname");
            peer.last_report.map(|x| {
                lua.push_number(x.to_msec() as f64);
                lua.set_field(idx, "last_report");
            });

            lua.set_field(peers, &id.to_hex()); // peer
        }
        lua.set_field(cfg, "peers");
    }
}
