use std::io;
use std::io::SeekFrom::{Current, Start};
use std::io::{copy, stdout, Seek};
use std::path::PathBuf;

use rustc_serialize::json::Json;
use tempfile::NamedTempFile;
use rumblebars::{eval, EvalContext};
use config::{Renderer, Command, ConfigSet};

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

pub fn render_all<'x>(renderers: &'x [Renderer], config: &ConfigSet,
    data: Json)
    -> Result<Vec<(NamedTempFile, &'x Command)>, Error>
{
    let mut result = Vec::new();
    for render in renderers {
        let template = try!(config.templates.get(&render.source)
            .ok_or(Error::TemplateNotFound(render.source.clone())));
        let mut tmpfile = try!(NamedTempFile::new());
        let ectx = EvalContext::new();
        try!(eval(template, &data, &mut tmpfile, &ectx));
        debug!("Rendered {:?} into {} bytes at {:?}",
            &render.source, tmpfile.seek(Current(0)).unwrap(), tmpfile.path());
        if config.options.print_configs {
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
