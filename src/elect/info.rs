use std::collections::HashMap;

use super::{Info, Id};


impl Info {
    pub fn new<S:AsRef<str>>(id: S) -> Info {
        Info {
            id: Id(id.as_ref().to_string()),
            all_hosts: HashMap::new(),
        }
    }
}
