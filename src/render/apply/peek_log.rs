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
        let expanded = variables.expand(&self.0);
        let path = Path::new(&expanded);
        if path.is_absolute() {
            task.log(
                format_args!("Warning: absolute paths are not suppported"));
            return Ok(());
        }
        let mut iter = path.iter();
        let first_cmp = match iter.next().and_then(|x| x.to_str()) {
            Some(x) => x,
            None => {
                task.log(format_args!("Invalid path {:?}", path));
                return Ok(());
            }
        };
        let tail = iter.as_path();
        let real_path = match task.sandbox.log_dirs.get(first_cmp) {
            Some(x) => x.join(tail),
            None => {
                task.log(format_args!("No directory named {:?} in sandbox \
                    config", first_cmp));
                return Ok(());
            }
        };
        task.log(format_args!("PeekLog {}/{:?}\n", first_cmp, &path));
        match fs::metadata(&real_path) {
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
