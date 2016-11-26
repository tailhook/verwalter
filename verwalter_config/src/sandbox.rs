use std::path::{Path, PathBuf};
use std::collections::HashMap;

use scan_dir::{ScanDir, Error as ScanDirError};
use quire::validate::{Structure, Mapping, Scalar};
use quire::{parse_config, Options, ErrorList};
use quick_error::ResultExt;

#[derive(RustcDecodable, Clone)]
pub struct Sandbox {
    pub log_dirs: HashMap<String, PathBuf>,
}

quick_error! {
    #[derive(Debug)]
    pub enum Error {
        Yaml(path: PathBuf, err: ErrorList) {
            display("parsing yaml {:?}: {}", path, err)
            description("error parsing yaml")
            context(p: AsRef<Path>, e: ErrorList)
                -> (p.as_ref().to_path_buf(), e)
        }
        ScanDir(err: Vec<ScanDirError>) {
            display("errors while scanning sandbox dir: {}",
                err.iter().map(|x| x.to_string())
                   .collect::<Vec<_>>().join("\n    "))
            description("error scanning sandbox dir")
            from()
        }
    }
}


impl Sandbox {
    pub fn validator() -> Structure<'static> {
        Structure::new()
        .member("log_dirs", Mapping::new(Scalar::new(), Scalar::new()))
    }
    /// Empty value used on verwalter_render --check-dir, because the config
    /// is irrelevant there
    pub fn empty() -> Sandbox {
        Sandbox {
            log_dirs: HashMap::new(),
        }
    }
    pub fn parse<P: AsRef<Path>>(p: P) -> Result<Sandbox, ErrorList> {
        parse_config(p.as_ref(), &Sandbox::validator(), &Options::default())
    }
    pub fn parse_all<P: AsRef<Path>>(dir: P) -> Result<Sandbox, Error> {
        ScanDir::files().walk(dir, |iter| {
            let mut config = Sandbox {
                log_dirs: HashMap::new(),
            };
            let filtered = iter.filter(|&(_, ref name)|
                name.ends_with(".yaml") || name.ends_with(".yml"));
            for (entry, _) in filtered {
                let path = entry.path();
                let new = try!(Sandbox::parse(&path).context(&path));

                // merge config
                config.log_dirs.extend(new.log_dirs.into_iter());
            }
            Ok(config)
        }).map_err(Error::ScanDir).and_then(|x| x)
    }

}
