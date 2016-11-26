use std::io;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::collections::HashMap;

use liquid::{self, Renderable};
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
        Handlebars(err: RenderError, file: PathBuf, data: Json) {
            display("error rendering template, file {:?}: \
                {}\n    data: {:?}", file, err, data)
            description("template rendering error")
            context(ctx: (&'a PathBuf, &'a Json), err: RenderError)
                -> (err, ctx.0.clone(), ctx.1.clone())
        }
        Liquid(err: liquid::Error, file: PathBuf, data: Json) {
            display("error rendering template, file {:?}: \
                {}\n    data: {:?}", file, err, data)
            description("template rendering error")
            context(ctx: (&'a PathBuf, &'a Json), err: liquid::Error)
                -> (err, ctx.0.clone(), ctx.1.clone())
        }
        NoSuchTemplate(file: PathBuf) {
            display("no such template {:?}", file)
            description("no such template")
        }
        UnknownTemplateType(file: PathBuf) {
            display("unknown template type {:?}", file)
            description("unknown template type")
        }
    }
}

fn to_liquid(j: &Json) -> liquid::Value {
    use rustc_serialize::json::Json::*;
    use liquid::Value as V;
    match *j {
        I64(v) => V::Num(v as f32),
        U64(v) => V::Num(v as f32),
        F64(v) => V::Num(v as f32),
        String(ref s) => V::Str(s.clone()),
        Boolean(v) => V::Bool(v),
        Array(ref v) => V::Array(v.iter().map(to_liquid).collect()),
        Object(ref o) => V::Object(o.iter()
            .map(|(k, v)| (k.clone(), to_liquid(v)))
            .collect()),
        Null => V::Str("".into()),
    }
}

fn set_vars_from_json(ctx: &mut liquid::Context, vars: &Json) {
    let vars = match *vars {
        Json::Object(ref ob) => ob,
        _ => unreachable!(),
    };
    for (key, val) in vars {
        ctx.set_val(key, to_liquid(val));
    }
}

pub fn render_role(dir: &Path, vars: &Json, log: &mut Role)
    -> Result<Vec<(String, Vec<Command>, Source)>, Error>
{
    let mut hbars = Handlebars::new();
    let mut liquid = HashMap::new();
    let mut result = Vec::new();

    let renderers = config::read_renderers(dir, &mut hbars, &mut liquid)?;
    let mut context = liquid::Context::new();
    if liquid.len() != 0 {
        set_vars_from_json(&mut context, vars);
    }

    for (rname, render) in renderers {
        let mut dest = HashMap::new();
        for (tname, tpath) in render.templates {
            let mut tmpfile = try!(NamedTempFile::new());

            let name = tpath.to_string_lossy().into_owned();
            let output = if name.ends_with(".hbs") ||
                    name.ends_with(".handlebars")
            {
                hbars.render(&name, vars).context((&tpath, vars))?
            } else if name.ends_with(".liquid") {
                liquid.get(&name)
                .ok_or_else(|| Error::NoSuchTemplate(tpath.clone()))?
                .render(&mut context)
                .context((&tpath, vars))?
                .unwrap_or_else(String::new)
            } else {
                return Err(Error::UnknownTemplateType(tpath));
            };
            tmpfile.write_all(output.as_bytes())?;
            log.template(&tpath, &tmpfile.path(), &output);
            dest.insert(tname.clone(), tmpfile);
        }
        result.push((rname, render.commands, Source::TmpFiles(dest)));
    }
    Ok(result)
}
