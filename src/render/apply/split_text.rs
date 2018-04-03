use std::collections::HashSet;
use std::io::{BufRead, BufReader, Write, BufWriter};
use std::fs::{self, File, remove_file};
use std::os::unix::fs::PermissionsExt;
use std::path::Path;

use capturing_glob::{self, glob_with, MatchOptions};
use failure::ResultExt;
use regex::Regex;
use serde_regex;
use quire::validate as V;

use apply::{Task, Error, Action};
use apply::expand::Variables;

#[derive(Deserialize, Debug, Clone)]
pub struct SplitText {
    #[serde(with="serde_regex")]
    section: Regex,
    #[serde(with="serde_regex")]
    validate: Regex,
    src: String,
    dest: String,
    mode: Option<u32>,
}

impl SplitText {
    pub fn config() -> V::Structure<'static> {
        V::Structure::new()
        .member("section", V::Scalar::new())
        .member("validate", V::Scalar::new())
        .member("src", V::Scalar::new().default("{{ tmp_file }}"))
        .member("dest", V::Scalar::new())
        .member("mode", V::Numeric::new().optional())
    }
}

fn open_file(dest: &str, name: &str) -> Result<File, Error> {
    let fpath = capturing_glob::Pattern::new(dest)
        .map_err(|_| format_err!(
            "Split test destination is invalid pattern: {:?}",
            dest))?
        .substitute(&[name])
        .map_err(|_| format_err!(
            "Split test destination is invalid pattern: {:?}",
            dest))?;
    let fname = Path::new(&fpath).file_name()
        .ok_or_else(|| format_err!(
            "SplitText destination must be filename \
             pattern not a directory: {:?}",
            dest))?;
    let tmpdest = Path::new(&fpath).with_file_name(
        format!(".tmp.{}", fname.to_str().unwrap()));
    Ok(File::create(&tmpdest).context(tmpdest.display().to_string())?)
}

fn commit_file(dest: &str, name: &str, mode: Option<u32>) -> Result<(), Error>
{
    let fpath = capturing_glob::Pattern::new(dest)
        .map_err(|_| format_err!(
            "Split test destination is invalid pattern: {:?}",
            dest))?
        .substitute(&[name])
        .map_err(|_| format_err!(
            "Split test destination is invalid pattern: {:?}",
            dest))?;
    let fname = Path::new(&fpath).file_name()
        .ok_or_else(|| format_err!(
            "SplitText destination must be filename \
             pattern not a directory: {:?}",
            dest))?;
    let tmpdest = Path::new(&fpath).with_file_name(
        format!(".tmp.{}", fname.to_str().unwrap()));
    if let Some(mode) = mode {
        fs::set_permissions(&tmpdest, fs::Permissions::from_mode(mode))
            .map_err(|e| format_err!("can't set permissions: {}", e))?;
    }
    fs::rename(&tmpdest, &fpath)
        .map_err(|e| format_err!("can't rename: {}", e))?;
    Ok(())
}


impl Action for SplitText {
    fn execute(&self, task: &mut Task, variables: &Variables)
        -> Result<(), Error>
    {
        let src_fn = variables.expand(&self.src);
        let dest = variables.expand(&self.dest);
        task.log(format_args!("SplitText {{ src: {:?}, dest: {:?} }}\n",
            &self.src, &self.dest));

        if !task.dry_run {
            let src = BufReader::new(File::open(&src_fn)
                .map_err(|e| format_err!("can't open {:?}: {}", src_fn, e))?);
            let mut visited = HashSet::new();
            let mut file = None;
            let mut name = None::<String>;
            for (num, line) in src.lines().enumerate() {
                let num = num+1;
                let line = line.map_err(|e|
                    format_err!("can't read {:?}: {}", src_fn, e))?;
                if let Some(capt) = self.section.captures(&line) {
                    let sect = capt.get(1).map(|x| x.as_str()).unwrap_or("");
                    if !self.validate.is_match(sect) {
                        return Err(format_err!(
                            "invalid section {:?} on line {}", capt, num));
                    }

                    if let Some(cur_name) = name.take() {
                        commit_file(&dest, &cur_name, self.mode)?;
                    }
                    file = Some(BufWriter::new(open_file(&dest, sect)?));
                    name = Some(sect.to_string());
                    visited.insert(sect.to_string());
                } else if file.is_none() {
                    if !line.trim().is_empty() {
                        return Err(format_err!(
                            "Error splitting file: \
                            Non-empty line {} before title", num));
                    }
                } else {
                    writeln!(&mut file.as_mut().unwrap(), "{}", line)
                        .map_err(|e| format_err!(
                            "can't section {:?}: {}", name, e))?;
                }
            }
            if let Some(cur_name) = name.take() {
                commit_file(&dest, &cur_name, self.mode)?;
            }
            let items = glob_with(&dest, &MatchOptions {
                case_sensitive: true,
                require_literal_separator: true,
                require_literal_leading_dot: true,
            }).map_err(|_| format_err!(
                "Split test destination is invalid pattern: {:?}", dest))?;
            for entry in items {
                let entry = entry.map_err(|e| {
                    format_err!("{:?}: {}",
                        e.path().to_path_buf(), e.error())
                })?;
                let name = entry.group(1)
                    .and_then(|x| x.to_str()).unwrap_or("");
                if !visited.contains(name) {
                    remove_file(entry.path())?;
                }
            }
            Ok(())
        } else {
            Ok(())
        }
    }

}
