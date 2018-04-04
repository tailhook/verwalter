use std::collections::HashSet;
use std::io::{BufRead, BufReader};
use std::fs::{File, remove_file};

use capturing_glob::{glob_with, MatchOptions};
use quire::validate as V;

use apply::{Task, Error, Action};
use apply::expand::Variables;

#[derive(Deserialize, Debug, Clone)]
pub struct CleanFiles {
    pattern: String,
    keep_list: String,
}

impl CleanFiles {
    pub fn config() -> V::Structure<'static> {
        V::Structure::new()
        .member("pattern", V::Scalar::new())
        .member("keep_list", V::Scalar::new())
    }
}

impl Action for CleanFiles {
    fn execute(&self, task: &mut Task, variables: &Variables)
        -> Result<(), Error>
    {
        let pattern = variables.expand(&self.pattern);
        let keep_list = variables.expand(&self.keep_list);
        task.log(format_args!(
            "CleanFiles {{ pattern: {:?}, keep_list: {:?} }}\n",
            &pattern, &keep_list));

        let src = BufReader::new(File::open(&keep_list)
            .map_err(|e| format_err!("can't open {:?}: {}", keep_list, e))?);
        let keep_list = src.lines().collect::<Result<HashSet<_>, _>>()?;
        let items = glob_with(&pattern, &MatchOptions {
            case_sensitive: true,
            require_literal_separator: true,
            require_literal_leading_dot: true,
        }).map_err(|_| format_err!(
            "CleanFiles has invalid pattern: {:?}", pattern))?;

        for entry in items {
            let entry = entry.map_err(|e| {
                format_err!("{:?}: {}",
                    e.path().to_path_buf(), e.error())
            })?;
            let name = entry.group(1)
                .and_then(|x| x.to_str()).unwrap_or("");
            if !keep_list.contains(name) {
                let path = entry.path();
                if task.dry_run {
                    task.log.log(format_args!("would remove {:?}\n", path));
                } else {
                    task.log.log(format_args!("removing {:?}\n", path));
                    remove_file(path)?;
                }
            }
        }

        Ok(())
    }

}
