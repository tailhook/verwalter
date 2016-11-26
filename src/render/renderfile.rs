use std::io::{self, Read};
use std::fs::File;
use std::path::{Path, PathBuf};
use std::collections::HashMap;

use liquid;
use scan_dir;
use quire::validate as V;
use quire::{parse_config, Options, ErrorList};
use handlebars::{Handlebars, TemplateError as HandlebarsError};
use quick_error::ResultExt;

use apply;
use render::Renderer;


quick_error! {
    #[derive(Debug)]
    pub enum TemplateError {
        TemplateRead(err: io::Error, path: PathBuf) {
            context(path: AsRef<Path>, err: io::Error)
                -> (err, path.as_ref().to_path_buf())
            cause(err)
            display("error reading {:?}: {}", path, err)
            description("error reading template file")
        }
        HandlebarsParse(err: HandlebarsError, path: PathBuf) {
            context(path: AsRef<Path>, err: HandlebarsError)
                -> (err, path.as_ref().to_path_buf())
            cause(err)
            display("error reading {:?}: {}", path, err)
            description("error reading handlebars file")
        }
        LiquidParse(err: liquid::Error, path: PathBuf) {
            context(path: AsRef<Path>, err: liquid::Error)
                -> (err, path.as_ref().to_path_buf())
            cause(err)
            display("error reading {:?}: {}", path, err)
            description("error reading liquid file")
        }
        Config(err: ErrorList, path: PathBuf) {
            context(path: AsRef<Path>, err: ErrorList)
                -> (err, path.as_ref().to_path_buf())
            display("error reading {:?}: {}", path, err)
            description("error reading config from template dir")
        }
        ScanDir(err: scan_dir::Error) {
            from() cause(err)
            display("{}", err)
            description("error reading template directory")
        }
    }
}


fn command_validator<'x>() -> V::Enum<'x> {
    V::Enum::new()
    .option("RootCommand", apply::root_command::RootCommand::config())
    .option("Cmd", apply::cmd::Cmd::config())
    .option("Sh", apply::shell::Sh::config())
    .option("Copy", apply::copy::Copy::config())
    .option("PeekLog", apply::peek_log::PeekLog::config())
}

fn config_validator<'x>() -> V::Structure<'x> {
    V::Structure::new()
    .member("templates", V::Mapping::new(V::Scalar::new(), V::Scalar::new()))
    .member("commands", V::Sequence::new(command_validator()))
}

fn read_renderer(path: &Path, base: &Path)
    -> Result<(String, Renderer), TemplateError>
{
    let path_rel = path.strip_prefix(base).unwrap();
    let template_base = path_rel.parent().unwrap();
    let orig: Renderer = parse_config(path,
        &config_validator(), &Options::default())
        .context(path)?;
    Ok((path_rel.to_string_lossy().to_string(), Renderer {
            // Normalize path to be relative to base path
            // rather than relative to current subdir
        templates: orig.templates.into_iter()
            .map(|(name, path)| (name, template_base.join(path)))
            .collect(),
        commands: orig.commands,
    }))
}

pub fn read_renderers(path: &Path,
    hbars: &mut Handlebars, liquid: &mut HashMap<String, liquid::Template>)
    -> Result<Vec<(String, Renderer)>, TemplateError>
{
    let mut renderers = Vec::new();
    try!(scan_dir::ScanDir::files().walk(path, |iter| {
        for (entry, fname) in iter {
            if fname.ends_with(".hbs") || fname.ends_with(".handlebars") {
                let epath = entry.path();
                let mut buf = String::with_capacity(4096);
                let tname = epath
                    .strip_prefix(path).unwrap()
                    .to_string_lossy();
                let mut f = File::open(&epath).context(&epath)?;
                f.read_to_string(&mut buf).context(&epath)?;
                hbars.register_template_string(&tname, buf).context(&epath)?;
            } else if fname.ends_with(".liquid") {
                let epath = entry.path();
                let mut buf = String::with_capacity(4096);
                let mut f = File::open(&epath).context(&epath)?;
                f.read_to_string(&mut buf).context(&epath)?;
                let template = liquid::parse(&buf,
                    liquid::LiquidOptions::default())
                    .context(&epath)?;
                let tname = epath
                    .strip_prefix(path).unwrap()
                    .to_string_lossy();
                liquid.insert(tname.into_owned(), template);
            } else if fname.ends_with(".render.yaml") ||
                      fname.ends_with(".render.yml")
            {
                let epath = entry.path();
                let rnd = try!(read_renderer(&epath, path));
                renderers.push(rnd);
            } else {
                // debug!("Ignored file {:?}", entry.path());
            }
        }
        Ok(())
    })
    .map_err(|mut v| TemplateError::ScanDir(v.pop().unwrap()))
    .and_then(|x| x));
    Ok(renderers)
}
