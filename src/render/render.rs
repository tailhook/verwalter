use std::io;
use std::default::Default;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::collections::HashMap;

use tempfile::NamedTempFile;
use handlebars::{Handlebars, RenderError as HbsError};
use tera::Error as TeraError;

use apply::{Source, Command};
use error_chain::ChainedError;
use indexed_log::Role;
use quick_error::ResultExt;
use renderfile::{self as config, TemplateError};
use serde_json::{Value, to_string};
use tera::Tera;


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
        UnknownTemplateType(path: PathBuf) {
            display("unknown template type {:?}", path)
            description("unknown template type")
        }
        Handlebars(err: HbsError, file: PathBuf, data: Value) {
            cause(err)
            display("error rendering template, file {:?}: \
                {}\n    data: {:?}", file, err, data)
            description("template rendering error")
            context(ctx: (&'a PathBuf, &'a Value), err: HbsError)
                -> (err, ctx.0.clone(), ctx.1.clone())
        }
        Tera(err: TeraError, file: PathBuf, data: Value) {
            cause(err)
            display("error rendering template, file {:?}: \
                {}\nData: {}", file, err.display(),
                to_string(data).unwrap_or_else(|_| String::from("bad data")))
            description("template rendering error")
            context(ctx: (&'a PathBuf, &'a Value), err: TeraError)
                -> (err, ctx.0.clone(), ctx.1.clone())
        }
    }
}

pub fn render_role(dir: &Path, vars: &Value, log: &mut Role)
    -> Result<Vec<(String, Vec<Command>, Source)>, Error>
{
    let mut hbars = Handlebars::new();
    let mut tera = Tera::default();
    let mut result = Vec::new();

    let renderers = config::read_renderers(dir, &mut hbars, &mut tera)?;

    for (rname, render) in renderers {
        let mut dest = HashMap::new();
        for (tname, tpath) in render.templates {
            let mut tmpfile = try!(NamedTempFile::new());

            let output = match tpath.extension().and_then(|x| x.to_str()) {
                Some("hbs") | Some("handlebars") => {
                    hbars.render(&tpath.display().to_string(), vars)
                        .context((&tpath, vars))?
                }
                Some("tera") => {
                    tera.render(&tpath.display().to_string(), vars)
                        .context((&tpath, vars))?
                }
                _ => {
                    return Err(
                        Error::UnknownTemplateType(tpath.to_path_buf()));
                }
            };
            tmpfile.write_all(output.as_bytes())?;
            log.template(&tpath, &tmpfile.path(), &output);
            dest.insert(tname.clone(), tmpfile);
        }
        result.push((rname, render.commands, Source::TmpFiles(dest)));
    }
    Ok(result)
}
