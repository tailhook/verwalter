use std::collections::HashMap;

use super::ExternalData;


impl ExternalData {
    pub fn empty() -> ExternalData {
        ExternalData {
            all_hosts: HashMap::new(),
        }
    }
}
