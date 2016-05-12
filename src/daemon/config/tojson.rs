use std::collections::{BTreeMap};

use rustc_serialize::json::{Json, ToJson};


trait JsonObject {
    fn add<K: ToString, V:ToJson>(self, k: K, v: V) -> Self;
}

impl JsonObject for BTreeMap<String, Json> {
    fn add<K: ToString, V:ToJson>(mut self, k: K, v: V) -> Self {
        self.insert(k.to_string(), v.to_json());
        self
    }
}
