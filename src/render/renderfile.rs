use std::collections::HashMap;
use std::fs::File;
use std::io::{self, Read};
use std::path::{Path, PathBuf};

use error_chain::ChainedError;
use handlebars::{Handlebars, TemplateError as HandlebarsError};
use quick_error::ResultExt;
use quire::{parse_config, Options, ErrorList as ConfigError};
use quire::validate as V;
use scan_dir;
use tera::{Tera, Error as TeraError};
use trimmer::{self, Template as Trimmer, ParseError as TrimmerError};

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
            display("error reading {:?}: {}", path, err.display())
            description("error reading template file")
            context(path: &'a Path, e: TeraError)
                -> (e, path.to_path_buf())
        }
        Trimmer(err: TrimmerError, path: PathBuf) {
            cause(err)
            display("error reading {:?}: {}", path, err)
            description("error reading template file")
            context(path: &'a Path, e: TrimmerError)
                -> (e, path.to_path_buf())
        }
        Config(err: ConfigError) {
            from()
            display("{}", err)
            description("error reading config from template dir")
        }
        ScanDir(err: scan_dir::Error) {
            from() cause(err)
            display("{}", err)
            description("error reading template directory")
        }
    }
}


pub fn command_validator<'x>(root: bool) -> V::Enum<'x> {
    let mut val = V::Enum::new()
        .option("RootCommand", apply::root_command::RootCommand::config())
        .option("Cmd", apply::cmd::Cmd::config())
        .option("Sh", apply::shell::Sh::config())
        .option("Copy", apply::copy::Copy::config())
        .option("SplitText", apply::split_text::SplitText::config())
        .option("PeekLog", apply::peek_log::PeekLog::config());
    if root {
        val = val.option("Condition", apply::condition::Condition::config())
    }
    return val;
}

fn config_validator<'x>() -> V::Structure<'x> {
    V::Structure::new()
    .member("templates", V::Mapping::new(V::Scalar::new(), V::Scalar::new()))
    .member("commands", V::Sequence::new(command_validator(true)))
}

fn read_renderer(path: &Path, base: &Path)
    -> Result<(String, Renderer), TemplateError>
{
    let path_rel = path.strip_prefix(base).unwrap();
    let template_base = path_rel.parent().unwrap();
    let orig: Renderer = parse_config(&path,
            &config_validator(), &Options::default())?;
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
    hbars: &mut Handlebars, tera: &mut Tera,
    trm: &mut HashMap<String, Trimmer>)
    -> Result<Vec<(String, Renderer)>, TemplateError>
{
    let trm_parser = trimmer::Parser::new();
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
            } else if fname.ends_with(".trm") || fname.ends_with(".trimmer") {
                let epath = entry.path();
                let mut buf = String::with_capacity(4096);
                let tname = epath
                    .strip_prefix(path).unwrap()
                    .to_string_lossy();
                File::open(&epath)
                    .and_then(|mut f| f.read_to_string(&mut buf))
                    .context(path)?;
                trm.insert(tname.to_string(),
                    trm_parser.parse(&buf).context(path)?);
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
