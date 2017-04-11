use std::io::{self, Read};
use std::fs::File;
use std::path::{Path, PathBuf};

use handlebars::{Handlebars, TemplateError as HandlebarsError};
use quire::sky::parse_config;
use quire::validate as V;
use scan_dir;
use tera::{Tera, Error as TeraError};
use quick_error::ResultExt;

use apply;
use render::Renderer;


quick_error! {
    #[derive(Debug)]
    pub enum TemplateError {
        TemplateRead(err: io::Error, path: PathBuf) {
            cause(err)
            display("error reading {:?}: {}", path, err)
            description("error reading template file")
            context(path: &'a Path, e: io::Error) -> (e, path.to_path_buf())
        }
        Handlebars(err: HandlebarsError, path: PathBuf) {
            cause(err)
            display("error reading {:?}: {}", path, err)
            description("error reading template file")
            context(path: &'a Path, e: HandlebarsError)
                -> (e, path.to_path_buf())
        }
        Tera(err: TeraError, path: PathBuf) {
            cause(err)
            display("error reading {:?}: {}", path, err)
            description("error reading template file")
            context(path: &'a Path, e: TeraError)
                -> (e, path.to_path_buf())
        }
        Config(err: String, path: PathBuf) {
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
    let orig: Renderer = try!(parse_config(&path,
        &config_validator(), Default::default())
        .map_err(|e| TemplateError::Config(e, path.to_path_buf())));
    Ok((path_rel.to_string_lossy().to_string(), Renderer {
            // Normalize path to be relative to base path
            // rather than relative to current subdir
        templates: orig.templates.into_iter()
            .map(|(name, path)| (name, template_base.join(path)))
            .collect(),
        commands: orig.commands,
    }))
}

pub fn read_renderers(path: &Path, hbars: &mut Handlebars, tera: &mut Tera)
    -> Result<Vec<(String, Renderer)>, TemplateError>
{
    let mut renderers = Vec::new();
    try!(scan_dir::ScanDir::files().walk(path, |iter| {
        for (entry, fname) in iter {
            if fname.ends_with(".hbs") || fname.ends_with(".handlebars")
            {
                let epath = entry.path();
                let mut buf = String::with_capacity(4096);
                let tname = epath
                    .strip_prefix(path).unwrap()
                    .to_string_lossy();
                File::open(&epath)
                    .and_then(|mut f| f.read_to_string(&mut buf))
                    .context(path)?;
                hbars.register_template_string(&tname, buf).context(path)?;
            } else if fname.ends_with(".tera") {
                let epath = entry.path();
                let mut buf = String::with_capacity(4096);
                let tname = epath
                    .strip_prefix(path).unwrap()
                    .to_string_lossy();
                File::open(&epath)
                    .and_then(|mut f| f.read_to_string(&mut buf))
                    .context(path)?;
                tera.add_raw_template(&tname, &buf).context(path)?;
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
