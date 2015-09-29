use std::i64;

use lua::{ToLua, State};
use rustc_serialize::json::Json;


pub fn push_json(lua: &mut State, json: &Json) {
    use rustc_serialize::json::Json::*;
    match json {
        &I64(v) => lua.push_integer(v),
        &U64(v) if v <= i64::MAX as u64 => {
            lua.push_integer(v as i64);
        }
        &U64(v) => {
            warn!("Too big integer for lua {}", v);
            // Unfortunately can't report error
            lua.push_nil();
        }
        &F64(v) => lua.push_number(v),
        &String(ref v) => lua.push_string(v),
        &Boolean(v) => lua.push_bool(v),
        &Array(ref v) => {
            lua.create_table(v.len() as i32, 0);
            let tbl = lua.get_top();
            for (i, item) in v.iter().enumerate() {
                push_json(lua, item);
                lua.raw_seti(tbl, i as i64);
            }
        },
        &Object(ref v) => {
            lua.create_table(0, v.len() as i32);
            let tbl = lua.get_top();
            for (key, val) in v.iter() {
                push_json(lua, val);
                lua.set_field(tbl, &key);
            }
        }
        &Null => lua.push_nil(),
    }
}
