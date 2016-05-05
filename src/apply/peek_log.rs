use std::fs;
use std::path::Path;

use quire::validate as V;
use rustc_serialize::json::{Json, ToJson};

use apply::{Task, Error, Action};
use apply::expand::Variables;

#[derive(Debug, Clone)]
pub struct PeekLog(String);
tuple_struct_decode!(PeekLog);

impl PeekLog {
    pub fn config() -> V::Scalar {
        V::Scalar::new()
    }
}

impl Action for PeekLog {
    fn execute(&self, mut task: Task, variables: Variables)
        -> Result<(), Error>
    {
        let path = variables.expand(&self.0);
        task.log(format_args!("PeekLog {:?}\n", &path));
        match fs::metadata(&path) {
            Ok(p) => {
                task.log.external_log(&Path::new(&path), p.len());
            }
            Err(e) => {
                task.log(format_args!("Log peek error: {:?}\n", e));
                // Always succeed. It's fine if log does not exist
            }
        }
        Ok(())
    }

}

impl ToJson for PeekLog {
    fn to_json(&self) -> Json {
        Json::Array(vec!["PeekLog".to_json(), self.0.to_json()])
    }
}
