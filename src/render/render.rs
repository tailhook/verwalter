use std::io;
use std::io::Write;
use std::path::Path;

use tempfile::NamedTempFile;
use handlebars::{Handlebars, RenderError};
use rustc_serialize::json::Json;

use config::{self, TemplateError};
use apply::{Source, Command};
use indexed_log::Role;

#[derive(RustcDecodable, Debug)]
pub struct Renderer {
    pub source: String,
    pub commands: Vec<Command>,
}

quick_error! {
    #[derive(Debug)]
    pub enum Error {
        Io(err: io::Error) {
            from() cause(err)
            display("I/O error: {}", err)
            description("I/O error")
        }
        BadTemplates(err: TemplateError) {
            from() cause(err)
            display("bad templates, errors: {}", err)
            description("couldn't parse templates")
        }
        Render(err: RenderError, file: String,
               data: Json) {
            display("error rendering template, file {:?}: \
                {}\n    data: {:?}", file, err, data)
            description("template rendering error")
       }
    }
}

pub fn render_role(dir: &Path, vars: &Json, log: &mut Role)
    -> Result<Vec<(String, Vec<Command>, Source)>, Error>
{
    let mut hbars = Handlebars::new();
    let mut result = Vec::new();

    let renderers = try!(config::read_renderers(dir, &mut hbars));

    for (rname, render) in renderers {
        let mut tmpfile = try!(NamedTempFile::new());

        let output = try!(hbars.render(&render.source, vars)
            .map_err(|e| Error::Render(e,
                render.source.clone(), vars.clone())));
        try!(tmpfile.write_all(output.as_bytes()));
        log.template(&render.source, &tmpfile.path(), &output);
        result.push((rname, render.commands, Source::TmpFile(tmpfile)));
    }
    Ok(result)
}
