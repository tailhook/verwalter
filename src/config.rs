use std::io;
use std::io::Read;
use std::fs::File;
use std::path::{Path, PathBuf};
use std::collections::HashMap;

use quire::validate as V;
use quire::sky::parse_config;
use walker::Walker;
use rumblebars::{Template, ParseError};
use path_util::relative;


quick_error! {
    #[derive(Debug)]
    pub enum Error {
        Io(err: io::Error) {
            from() cause(err)
            display("I/O error: {}", err)
        }
        Template(path: PathBuf, err: ParseError, descr: Option<String>) {
            display("error compiling {:?}: {:?} {}",
                path, err, descr.as_ref().unwrap_or(&String::from("")))
        }
        Config(path: PathBuf, err: String) {
            display("error reading {:?}: {}", path, err)
        }
    }
}


#[derive(RustcDecodable, Debug, Clone)]
pub enum Command {
    RootCommand(Vec<String>),
}

#[derive(RustcDecodable, Debug, Clone)]
pub struct Renderer {
    pub source: PathBuf,
    pub apply: Command,
}

#[derive(RustcDecodable, Debug)]
struct Config {
    render: Vec<Renderer>,
}

fn validator<'x>() -> V::Structure<'x> {
    V::Structure::new()
    .member("render", V::Sequence::new(
        V::Structure::new()
        .member("source", V::Scalar::new())
        .member("apply", V::Enum::new().optional()
            .option("RootCommand",
                V::Sequence::new(V::Scalar::new())
                //.from_scalar(..)
            ))
        ))
}

pub struct ConfigSet {
    pub templates: HashMap<PathBuf, Template>,
    pub renderers: Vec<Renderer>,
}

pub fn read_configs(path: &Path) -> Result<ConfigSet, Error>
{
    let mut cfg = ConfigSet {
        templates: HashMap::new(),
        renderers: Vec::new(),
    };
    let cfgv = validator();
    debug!("Configuration directory: {:?}", path);
    for entry in try!(Walker::new(path)) {
        let epath = try!(entry).path();
        let hidden = path.file_name().and_then(|x| x.to_str())
            .map(|x| x.starts_with(".")).unwrap_or(false);
        if hidden {
            continue;
        }
        let rpath = relative(&epath, path).unwrap();
        // The last (shortest) extension
        match epath.extension().and_then(|x| x.to_str()) {
            Some("hbs") | Some("handlebars") => {
                let mut buf = String::with_capacity(4096);
                try!(File::open(&epath)
                     .and_then(|mut x| x.read_to_string(&mut buf)));
                debug!("Adding template {:?}", epath);
                cfg.templates.insert(rpath.to_path_buf(),
                    try!(buf.parse().map_err(|(e, txt)|
                        Error::Template(epath.clone(), e, txt))));
            }
            _ => {}
        }
        // The longest extension
        match epath.to_str().and_then(|x| x.splitn(2, ".").nth(1)) {
            Some("vw.yaml") => {
                // read config
                debug!("Reading config {:?}", epath);
                let piece: Config = try!(parse_config(&epath,
                                            &cfgv, Default::default())
                    .map_err(|e| Error::Config(epath.clone(), e)));
                cfg.renderers.extend(piece.render.into_iter()
                .map(|r| Renderer {
                    // Normalize path to be relative to config root rather
                    // than relative to current subdir
                    source: relative(
                        &epath.parent().unwrap().join(r.source),
                        path).unwrap().to_path_buf(),
                    apply: r.apply,
                }));
            }
            _ => {}
        }
    }
    Ok(cfg)
}
