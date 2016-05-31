use std::io;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::collections::HashMap;

use tempfile::NamedTempFile;
use handlebars::{Handlebars, RenderError};
use rustc_serialize::json::Json;

use renderfile::{self as config, TemplateError};
use apply::{Source, Command};
use indexed_log::Role;
use quick_error::ResultExt;

#[derive(RustcDecodable, Debug)]
pub struct Renderer {
    pub templates: HashMap<String, PathBuf>,
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
        Render(err: RenderError, file: PathBuf, data: Json) {
            display("error rendering template, file {:?}: \
                {}\n    data: {:?}", file, err, data)
            description("template rendering error")
            context(ctx: (&'a PathBuf, &'a Json), err: RenderError)
                -> (err, ctx.0.clone(), ctx.1.clone())
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
        let mut dest = HashMap::new();
        for (tname, tpath) in render.templates {
            let mut tmpfile = try!(NamedTempFile::new());

            let output = try!(hbars.render(&tpath.display().to_string(), vars)
                .context((&tpath, vars)));
            try!(tmpfile.write_all(output.as_bytes()));
            log.template(&tpath, &tmpfile.path(), &output);
            dest.insert(tname.clone(), tmpfile);
        }
        result.push((rname, render.commands, Source::TmpFiles(dest)));
    }
    Ok(result)
}
