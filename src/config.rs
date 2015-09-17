use std::io;
use std::io::Read;
use std::fs::File;
use std::path::{Path, PathBuf};
use std::collections::HashMap;

use quire::validate as V;
use quire::sky::parse_config;
use walker::Walker;
use rumblebars::{Template, ParseError};


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
enum Command {
    RootCommand(Vec<String>),
}

#[derive(RustcDecodable, Debug, Clone)]
struct Renderer {
    source: PathBuf,
    apply: Command,
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
    templates: HashMap<PathBuf, Template>,
}


pub fn read_configs(path: &Path) -> Result<ConfigSet, Error>
{
    let mut cfg = ConfigSet {
        templates: HashMap::new(),
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
        if let Some(spath) = epath.to_str() {
            match epath.to_str().and_then(|x| x.splitn(2, ".").nth(1)) {
                Some("hbs") | Some("handlebars") => {
                    let mut buf = String::with_capacity(4096);
                    try!(File::open(&epath)
                         .and_then(|mut x| x.read_to_string(&mut buf)));
                    debug!("Adding template {:?}", epath);
                    cfg.templates.insert(epath.clone(),
                        try!(buf.parse().map_err(|(e, txt)|
                            Error::Template(epath.clone(), e, txt))));
                }
                Some("vw.yaml") => {
                    // read config
                    let cfg: Config = try!(parse_config(&epath,
                                                &cfgv, Default::default())
                        .map_err(|e| Error::Config(epath.clone(), e)));
                    debug!("Reading config {:?}", epath);
                }
                _ => {}
            }
        }
    }
    Ok(cfg)
}
