use std::sync::Arc;
use std::collections::{HashMap, BTreeMap};

use time::get_time;
use lua::{ToLua, State};
use rotor_cantal::{RemoteQuery, Dataset, Key, Value, Chunk, TimeStamp};
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
    pub metrics: Option<Arc<RemoteQuery>>,
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

        if let Some(ref metrics) = self.metrics {
            lua.new_table();
            let mtable = lua.get_top();
            for (host, data) in metrics.items.iter() {
                lua.new_table();
                let htable = lua.get_top();
                for (key, met) in data {
                    store_metric(lua, met);
                    lua.set_field(htable, key);
                }
                lua.set_field(mtable, host);
            }
            lua.set_field(cfg, "metrics");
        } else {
            println!("-------------------- no metrics -------------------");
        }
    }
}

fn store_key(lua: &mut State, key: &Key) {
    use rotor_cantal::KeyVisitor::{Key, Value};
    lua.new_table();
    let tbl = lua.get_top();
    key.visit(|x| {
        match x {
            Key(k) => lua.push_string(k),
            Value(v) => {
                lua.push_string(v);
                lua.set_table(tbl);
            }
        }
    });
}

fn store_value(lua: &mut State, value: &Value) {
    use rotor_cantal::Value::*;
    match *value {
        Counter(x) => lua.push_integer(x as i64), // TODO(tailhook) > i64::MAX?
        Integer(x) => lua.push_integer(x),
        Float(x) => lua.push_number(x),
        State(_) => unimplemented!(),
    }
}

fn store_chunk(lua: &mut State, value: &Chunk) {
    use rotor_cantal::Chunk::*;
    lua.new_table();
    let tbl = lua.get_top();
    match *value {
        Counter(ref vals) => {
            for (i, v) in vals.iter().enumerate() {
                match *v { // TODO(tailhook) x > i64::MAX?
                    Some(v) => lua.push_integer(v as i64),
                    None => lua.push_nil(),
                }
                lua.raw_seti(tbl, (i+1) as i64);
            }
        },
        Integer(ref vals) => {
            for (i, v) in vals.iter().enumerate() {
                match *v {
                    Some(v) => lua.push_integer(v),
                    None => lua.push_nil(),
                }
                lua.raw_seti(tbl, (i+1) as i64);
            }
        },
        Float(ref vals) => {
            for (i, v) in vals.iter().enumerate() {
                match *v {
                    Some(v) => lua.push_number(v),
                    None => lua.push_nil(),
                }
                lua.raw_seti(tbl, (i+1) as i64);
            }
        },
        State(_) => unimplemented!(),
    }
}

fn store_stamps(lua: &mut State, stamps: &Vec<TimeStamp>) {
    lua.new_table();
    let tbl = lua.get_top();
    for (i, ts) in stamps.iter().enumerate() {
        lua.push_integer(*ts as i64);
        lua.raw_seti(tbl, (i + 1) as i64);
    }
}

fn store_metric(lua: &mut State, metric: &Dataset) {
    use rotor_cantal::Dataset::*;
    match *metric {
        SingleSeries(ref key, ref chunk, ref stamps) => {
            lua.new_table();
            let tbl = lua.get_top();
            lua.push_string("single_series");
            lua.set_field(tbl, "type");
            store_key(lua, key);
            lua.set_field(tbl, "key");
            store_chunk(lua, chunk);
            lua.set_field(tbl, "values");
            store_stamps(lua, stamps);
            lua.set_field(tbl, "timestamps");
            // TODO(tailhook) push values
        },
        MultiSeries(ref items) => {
            lua.new_table();
            let tbl = lua.get_top();
            lua.push_string("multi_series");
            lua.set_field(tbl, "type");
            lua.new_table();
            let titems = lua.get_top();
            for (i, &(ref key, ref chunk, ref stamps))
                in items.iter().enumerate()
            {
                lua.new_table();
                let item = lua.get_top();
                store_key(lua, key);
                lua.set_field(item, "key");
                store_chunk(lua, chunk);
                lua.set_field(item, "values");
                store_stamps(lua, stamps);
                lua.set_field(item, "timestamps");
                lua.raw_seti(titems, (i+1) as i64);
            }
            lua.set_field(tbl, "items");
        },
        SingleTip(ref key, ref value, ref slc) => {
            lua.new_table();
            let tbl = lua.get_top();
            lua.push_string("single_tip");
            lua.set_field(tbl, "type");
            store_key(lua, key);
            lua.set_field(tbl, "key");
            store_value(lua, value);
            lua.set_field(tbl, "value");
            lua.push_integer(slc.0 as i64);
            lua.set_field(tbl, "old_timestamp");
            lua.push_integer(slc.1 as i64);
            lua.set_field(tbl, "new_timestamp");
        },
        MultiTip(ref items) => {
            lua.new_table();
            let tbl = lua.get_top();
            lua.push_string("multi_tip");
            lua.set_field(tbl, "type");
            lua.new_table();
            let titems = lua.get_top();
            for (i, &(ref key, ref value, ref timestamp))
                in items.iter().enumerate()
            {
                lua.new_table();
                let item = lua.get_top();
                store_key(lua, key);
                lua.set_field(item, "key");
                store_value(lua, value);
                lua.set_field(item, "value");
                lua.push_integer(timestamp.0 as i64);
                lua.set_field(item, "timestamp");
                lua.raw_seti(titems, (i+1) as i64);
            }
            lua.set_field(tbl, "items");
        }
        Chart(_) => unimplemented!(),
        Empty => lua.push_nil(),
        Incompatible(_) => {
            lua.new_table();
            let tbl = lua.get_top();
            lua.push_string("error");
            lua.set_field(tbl, "type");
            lua.push_string("incompatible");
            lua.set_field(tbl, "error");
        }
    }
}
