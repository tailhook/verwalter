use std::io;
use std::io::Read;
use std::fs::File;
use std::path::{Path, PathBuf};
use std::collections::HashMap;

use hlua::{Lua, LuaError};
use quire::validate as V;
use quire::sky::parse_config;
use walker::Walker;
use rumblebars::{Template, ParseError};
use path_util::relative;
use Options;


quick_error! {
    #[derive(Debug)]
    pub enum Error {
        DirScan(err: io::Error, path: PathBuf) {
            display("error scanning dir {:?}: {}", path, err)
        }
        CantReadLua(err: LuaError, path: PathBuf) {
            display("error reading lua script {:?}: {:?}", path, err)
        }
        ReadTemplate(err: io::Error, path: PathBuf) {
            cause(err)
            display("error reading {:?}: {}", path, err)
        }
        ParseTemplate(path: PathBuf, err: ParseError, descr: Option<String>) {
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
    pub variables: HashMap<String, String>,
}

// Configuration in .vw.yaml
#[derive(RustcDecodable, Debug)]
struct Config {
    render: Vec<Renderer>,
    variables: HashMap<String, String>,
}

// Configuration in meta.yaml
#[derive(RustcDecodable, Debug)]
struct Meta {
    variables: HashMap<String, String>,
}

fn config_validator<'x>() -> V::Structure<'x> {
    V::Structure::new()
    .member("render", V::Sequence::new(
        V::Structure::new()
        .member("source", V::Scalar::new())
        .member("variables", V::Mapping::new(V::Scalar::new(),
                                             V::Scalar::new()))
        .member("apply", V::Enum::new().optional()
            .option("RootCommand",
                V::Sequence::new(V::Scalar::new())
                //.from_scalar(..)
            ))
        ))
    .member("variables", V::Mapping::new(V::Scalar::new(), V::Scalar::new()))
}

fn meta_validator<'x>() -> V::Structure<'x> {
    V::Structure::new()
    .member("variables", V::Mapping::new(V::Scalar::new(), V::Scalar::new()))
}

pub struct ConfigSet<'lua> {
    pub options: Options,
    pub templates: HashMap<PathBuf, Template>,
    pub renderers: Vec<Renderer>,
    pub lua: Lua<'lua>,
}

fn read_meta(base: &Path)
    -> Result<HashMap<PathBuf, HashMap<String, String>>, Error>
{
    let cfgv = meta_validator();
    let mut dirvars = HashMap::new();
    for entry in try!(Walker::new(base)
                      .map_err(|e| Error::DirScan(e, base.to_path_buf())))
    {
        let entry = try!(entry
            .map_err(|e| Error::DirScan(e, base.to_path_buf())));
        if entry.file_name().to_str() == Some("meta.yaml") {
            let piece: Meta = try!(parse_config(&entry.path(),
                                                &cfgv, Default::default())
                .map_err(|e| Error::Config(entry.path(), e)));
            dirvars.insert(
                relative(&entry.path(), base).unwrap().to_path_buf(),
                piece.variables);
        }
    }
    Ok(dirvars)
}

fn read_scheduler(options: &Options, lua: &mut Lua) -> Result<(), Error> {
    let path = &options.config_dir.join("scheduler/main.lua");
    let f = try!(File::open(&path)
        .map_err(|e| Error::CantReadLua(LuaError::ReadError(e), path.clone())));
    try!(lua.execute_from_reader(f)
        .map_err(|e| Error::CantReadLua(e, path.clone())));
    Ok(())
}

fn update_missing<'x, I>(target: &mut HashMap<String, String>, src: I)
    where I: Iterator<Item=(&'x String, &'x String)>
{
    for (k, v) in src {
        if !target.contains_key(k) {
            target.insert(k.clone(), v.clone());
        }
    }
}

pub fn read_configs<'lua>(options: Options) -> Result<ConfigSet<'lua>, Error>
{
    let mut cfg = ConfigSet {
        options: options,
        templates: HashMap::new(),
        renderers: Vec::new(),
        lua: Lua::new(),
    };
    {
        let cfgdir = &cfg.options.config_dir;
        let dirvars = try!(read_meta(&cfgdir));
        let cfgv = config_validator();
        debug!("Configuration directory: {:?}", cfgdir);
        for entry in try!(Walker::new(cfgdir)
                          .map_err(|e| Error::DirScan(e, cfgdir.to_path_buf())))
        {
            let epath = try!(entry
                .map_err(|e| Error::DirScan(e, cfgdir.to_path_buf())))
                .path();
            let hidden = epath.file_name().and_then(|x| x.to_str())
                .map(|x| x.starts_with(".")).unwrap_or(false);
            if hidden {
                continue;
            }
            let rpath = relative(&epath, &cfgdir).unwrap();
            // The last (shortest) extension
            match epath.extension().and_then(|x| x.to_str()) {
                Some("hbs") | Some("handlebars") => {
                    let mut buf = String::with_capacity(4096);
                    try!(File::open(&epath)
                         .and_then(|mut x| x.read_to_string(&mut buf))
                         .map_err(|e| Error::ReadTemplate(e, epath.to_path_buf())));
                    debug!("Adding template {:?}", epath);
                    cfg.templates.insert(rpath.to_path_buf(),
                        try!(buf.parse().map_err(|(e, txt)|
                            Error::ParseTemplate(epath.clone(), e, txt))));
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

                    let mut vars = piece.variables;
                    let mut ppath: &Path = &epath;
                    loop {
                        if let Some(v) = dirvars.get(&ppath.to_path_buf()) {
                            update_missing(&mut vars, v.iter());
                        }
                        ppath = if let Some(path) = ppath.parent() { path }
                            else {
                                break;
                            };
                    }

                    cfg.renderers.extend(piece.render.into_iter()
                    .map(|r| {
                        let mut rvars = r.variables;
                        update_missing(&mut rvars, vars.iter());
                        Renderer {
                            // Normalize path to be relative to config root
                            // rather than relative to current subdir
                            source: relative(
                                &epath.parent().unwrap().join(r.source),
                                cfgdir).unwrap().to_path_buf(),
                            apply: r.apply,
                            variables: rvars,
                        }
                    }));
                }
                _ => {}
            }
        }
        try!(read_scheduler(&cfg.options, &mut cfg.lua))
    }
    Ok(cfg)
}
