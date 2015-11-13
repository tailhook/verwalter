use rustc_serialize::json::{Json, ToJson};

use config::Config;

impl ToJson for Config {
    fn to_json(&self) -> Json {
        Json::Object(vec![
            ("verwalter_version".to_string(),
                concat!("v", env!("CARGO_PKG_VERSION")).to_json()),
        ].into_iter().collect())
    }
}
