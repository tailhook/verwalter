use std::fs;
use std::path::Path;

use quire::validate as V;
use rustc_serialize::json::{Json, ToJson};

use apply::{Task, Error, Action};
use apply::expand::Variables;

#[derive(RustcDecodable, Debug, Clone)]
pub struct Copy {
    src: String,
    dest: String,
}

impl Copy {
    pub fn config() -> V::Structure<'static> {
        V::Structure::new()
        .member("src", V::Scalar::new().default("{{ tmp_file }}"))
        .member("dest", V::Scalar::new())
    }
}

impl Action for Copy {
    fn execute(&self, mut task: Task, variables: Variables)
        -> Result<(), Error>
    {
        let src = variables.expand(&self.src);
        let dest = variables.expand(&self.dest);
        task.log(format_args!("Copy {:#?}\n", &self));
        if !task.dry_run {
            let fname = try!(Path::new(&dest).file_name()
                .ok_or_else(|| Error::InvalidArgument(
                    "Copy destination must be filename not a directory",
                    dest.clone())));
            let tmpdest = Path::new(&dest).with_file_name(
                format!(".tmp.{}", fname.to_str().unwrap()));
            try!(fs::copy(&src, &tmpdest)
                .map_err(|e| {
                    task.log.log(format_args!(
                        "{:#?} failed to copy: {}\n", self, e));
                    Error::IoError(e)
                }));
            try!(fs::rename(&tmpdest, &dest)
                .map_err(|e| {
                    task.log.log(format_args!(
                        "{:#?} failed to rename: {}\n", self, e));
                    Error::IoError(e)
                }));
            Ok(())
        } else {
            Ok(())
        }
    }

}

impl ToJson for Copy {
    fn to_json(&self) -> Json {
        Json::Object(vec![
            ("src".to_string(), self.src.to_json()),
            ("dest".to_string(), self.dest.to_json()),
        ].into_iter().collect())
    }
}
