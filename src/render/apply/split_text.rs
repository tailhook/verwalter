use std::collections::HashSet;
use std::io::{BufRead, BufReader, Write, BufWriter};
use std::fs::{self, File, remove_file};
use std::os::unix::fs::PermissionsExt;
use std::path::Path;

use capturing_glob::{self, glob_with, MatchOptions};
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
        .map_err(|_| Error::InvalidArgument(
            "Split test destination is invalid pattern",
            dest.to_string()))?
        .substitute(&[name])
        .map_err(|_| Error::InvalidArgument(
            "Split test destination is invalid pattern",
            dest.to_string()))?;
    let fname = Path::new(&fpath).file_name()
        .ok_or_else(|| Error::InvalidArgument(
            "SplitText destination must be filename pattern not a directory",
            dest.to_string()))?;
    let tmpdest = Path::new(&fpath).with_file_name(
        format!(".tmp.{}", fname.to_str().unwrap()));
    return File::create(tmpdest).map_err(Error::IoError);
}

fn commit_file(dest: &str, name: &str, mode: Option<u32>) -> Result<(), Error>
{
    let fpath = capturing_glob::Pattern::new(dest)
        .map_err(|_| Error::InvalidArgument(
            "Split test destination is invalid pattern",
            dest.to_string()))?
        .substitute(&[name])
        .map_err(|_| Error::InvalidArgument(
            "Split test destination is invalid pattern",
            dest.to_string()))?;
    let fname = Path::new(&fpath).file_name()
        .ok_or_else(|| Error::InvalidArgument(
            "SplitText destination must be filename pattern not a directory",
            dest.to_string()))?;
    let tmpdest = Path::new(&fpath).with_file_name(
        format!(".tmp.{}", fname.to_str().unwrap()));
    if let Some(mode) = mode {
        fs::set_permissions(&tmpdest, fs::Permissions::from_mode(mode))
            .map_err(|e| Error::IoError(e))?;
    }
    fs::rename(&tmpdest, &fpath)
        .map_err(|e| Error::IoError(e))?;
    Ok(())
}


impl Action for SplitText {
    fn execute(&self, mut task: Task, variables: Variables)
        -> Result<(), Error>
    {
        let src = variables.expand(&self.src);
        let dest = variables.expand(&self.dest);
        task.log(format_args!("SplitText {{ src: {:?}, dest: {:?} }}\n",
            &self.src, &self.dest));

        if !task.dry_run {
            let src = BufReader::new(File::open(src).map_err(Error::IoError)?);
            let mut visited = HashSet::new();
            let mut file = None;
            let mut name = None::<String>;
            for (num, line) in src.lines().enumerate() {
                let line = line.map_err(Error::IoError)?;
                if file.is_none() {
                    if !line.trim().is_empty() {
                        return Err(Error::FormatError(format!(
                            "Error splitting file: \
                            Non-empty line {} before title", num)));
                    }
                } else if let Some(capt) = self.section.captures(&line) {
                    let sect = capt.get(1).map(|x| x.as_str()).unwrap_or("");
                    if !self.validate.is_match(sect) {
                        return Err(Error::FormatError(format!(
                            "invalid section {:?} on line {}", capt, num)));
                    }

                    if let Some(cur_name) = name.take() {
                        commit_file(&dest, &cur_name, self.mode)?;
                    }
                    file = Some(BufWriter::new(open_file(&dest, sect)?));
                    name = Some(sect.to_string());
                    visited.insert(sect.to_string());
                } else {
                    writeln!(&mut file.as_mut().unwrap(), "{}", line)
                        .map_err(|e| Error::IoError(e))?;
                }
            }
            if let Some(cur_name) = name.take() {
                commit_file(&dest, &cur_name, self.mode)?;
            }
            let items = glob_with(&dest, &MatchOptions {
                case_sensitive: true,
                require_literal_separator: true,
                require_literal_leading_dot: true,
            }).map_err(|_| Error::InvalidArgument(
                "Split test destination is invalid pattern",
                dest.to_string()))?;
            for entry in items {
                let entry = entry.map_err(|e| {
                    Error::Other(format!("{:?}: {}",
                        e.path().to_path_buf(), e.error()))
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
