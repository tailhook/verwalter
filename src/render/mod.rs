use std::io;
use std::io::SeekFrom::{Current, Start};
use std::io::{copy, stdout, Seek};
use std::path::PathBuf;

use tempfile::NamedTempFile;
use handlebars::Handlebars;
use rustc_serialize::json::Json;


pub struct RenderSet {
    pub items: Vec<Renderer>,
    pub handlebars: Handlebars,
}

impl ::std::fmt::Debug for RenderSet {
    fn fmt(&self, fmt: &mut ::std::fmt::Formatter) -> ::std::fmt::Result {
        fmt.debug_map()
            .entry(&"renderers", &self.items)
            .entry(&"handlebars", &"Handlebars")
            .finish()
    }
}

#[derive(RustcDecodable, Debug)]
pub struct Renderer {
    pub source: String,
    pub apply: Command,
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
