use std::io;
use std::io::SeekFrom::{Current, Start};
use std::io::{copy, stdout, Seek};
use std::path::PathBuf;
use std::collections::HashMap;

use tempfile::NamedTempFile;
use rustc_serialize::json::Json;

use super::config::Template;


#[derive(Debug)]
pub struct RenderSet {
    items: Vec<Renderer>
}

#[derive(Debug)]
pub struct Renderer {
    pub source: Template,
    pub apply: Command,
    pub variables: HashMap<String, String>,
}

#[derive(RustcDecodable, Debug, Clone)]
pub enum Command {
    RootCommand(Vec<String>),
}

quick_error! {
    #[derive(Debug)]
    pub enum Error {
        Io(err: io::Error) {
            from() cause(err)
            display("I/O error: {}", err)
        }
        TemplateNotFound(path: PathBuf) {
            display("Can't find template: {:?}", path)
        }
    }
}

pub fn render_all<'x>(set: &'x RenderSet, data: Json, print: bool)
    -> Result<Vec<(NamedTempFile, &'x Command)>, Error>
{
    let mut result = Vec::new();
    for render in &set.items {
        let mut tmpfile = try!(NamedTempFile::new());
        debug!("Rendered {:?} into {} bytes at {:?}",
            &render.source,
            tmpfile.seek(Current(0)).unwrap(), tmpfile.path());
        if print {
            println!("----- [ {:?} -> {:?} ] -----",
                render.source, tmpfile.path());
            tmpfile.seek(Start(0)).unwrap();
            try!(copy(&mut tmpfile, &mut stdout()));
            println!("----- End of [ {:?} -> {:?} ] -----",
                render.source, tmpfile.path());
        }
        result.push((tmpfile, &render.apply));
    }
    Ok(result)
}
