use std::collections::{BTreeMap, HashMap};

use rustc_serialize::json::{Json, ToJson};

use config::{Config, Role, MetadataErrors, Version};
use render::{RenderSet, Renderer};

trait JsonObject {
    fn add<K: ToString, V:ToJson>(self, k: K, v: V) -> Self;
}

impl JsonObject for BTreeMap<String, Json> {
    fn add<K: ToString, V:ToJson>(mut self, k: K, v: V) -> Self {
        self.insert(k.to_string(), v.to_json());
        self
    }
}

impl ToJson for Config {
    fn to_json(&self) -> Json {
        Json::Object(BTreeMap::new()
        .add("verwalter_version",
            concat!("v", env!("CARGO_PKG_VERSION")).to_json())
        .add("machine", match self.machine {
            Ok(ref meta) => meta.to_json(),
            Err(ref err) => err.to_json(),
        })
        .add("roles", self.roles.to_json()))
    }
}

impl ToJson for Role {
    fn to_json(&self) -> Json {
        Json::Object(BTreeMap::new()
        .add("renderers", self.renderers.iter()
            .map(|(ref k, ref v)| {
                (k.0.clone(), v.as_ref()
                    .map(|x| x.to_json())
                    .unwrap_or_else(|elist| {
                        Json::Object(BTreeMap::new()
                        .add("errors", elist.errors.iter()
                            .map(|x| x.to_string())
                            .collect::<Vec<_>>()))
                        }))
            })
            .collect::<BTreeMap<_, _>>())
        .add("runtime", self.runtime.iter()
            .map(|(ref k, ref v)| {
                (k.0.clone(), v.as_ref()
                    .map(|x| x.to_json())
                    .unwrap_or_else(|elist| {
                        Json::Object(BTreeMap::new()
                        .add("errors", elist.errors.iter()
                            .map(|x| x.to_string())
                            .collect::<Vec<_>>()))
                        }))
            })
            .collect::<BTreeMap<_, _>>()))
    }
}

impl ToJson for MetadataErrors {
    fn to_json(&self) -> Json {
        Json::Object(BTreeMap::new()
        .add("errors", self.errors.iter()
            .map(|x| x.to_string().to_json())
            .collect::<Vec<_>>()))
    }
}

impl ToJson for RenderSet {
    fn to_json(&self) -> Json {
        Json::Object(BTreeMap::new()
        .add("items", self.items.to_json()))
    }
}

impl ToJson for Renderer {
    fn to_json(&self) -> Json {
        Json::Object(BTreeMap::new()
        .add("source", self.source.to_json())
        .add("apply", self.apply.to_json()))
    }
}
