use std::fs;
use std::path::Path;
use std::os::unix::fs::PermissionsExt;

use quire::validate as V;

use apply::{Task, Error, Action};
use apply::expand::Variables;

#[derive(Deserialize, Debug, Clone)]
pub struct Copy {
    src: String,
    dest: String,
    mode: Option<u32>,
}

impl Copy {
    pub fn config() -> V::Structure<'static> {
        V::Structure::new()
        .member("src", V::Scalar::new().default("{{ tmp_file }}"))
        .member("dest", V::Scalar::new())
        .member("mode", V::Numeric::new().optional())
    }
}

impl Action for Copy {
    fn execute(&self, task: &mut Task, variables: &Variables)
        -> Result<(), Error>
    {
        let src = variables.expand(&self.src);
        let dest = variables.expand(&self.dest);
        task.log(format_args!("Copy {{ src: {:?}, dest: {:?} }}\n",
            &self.src, &self.dest));
        if !task.dry_run {
            let fname = Path::new(&dest).file_name()
                .ok_or_else(|| format_err!(
                    "Copy destination must be filename not a directory: {:?}",
                    dest))?;
            let tmpdest = Path::new(&dest).with_file_name(
                format!(".tmp.{}", fname.to_str().unwrap()));
            fs::copy(&src, &tmpdest)
                .map_err(|e| {
                    task.log.log(format_args!(
                        "{:?} failed to copy: {}\n", self, e));
                    format_err!("{:?} failed to copy: {}\n", self, e)
                })?;
            if let Some(mode) = self.mode {
                fs::set_permissions(&tmpdest, fs::Permissions::from_mode(mode))
                    .map_err(|e| {
                        task.log.log(format_args!(
                            "{:?} failed to set mode: {}\n", self, e));
                        format_err!("{:?} failed to set mode: {}\n", self, e)
                    })?;
            }
            fs::rename(&tmpdest, &dest)
                .map_err(|e| {
                    task.log.log(format_args!(
                        "{:?} failed to rename: {}\n", self, e));
                    format_err!(
                        "{:?} failed to rename: {}\n", self, e)
                })?;
            Ok(())
        } else {
            Ok(())
        }
    }

}
