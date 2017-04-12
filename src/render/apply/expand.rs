use std::collections::HashMap;

use regex::{Regex, Captures};
use apply;


lazy_static! {
    static ref VAR_REGEX: Regex = {
        Regex::new(r"\{\{\s*([\w\.]+)\s*\}\}")
        .expect("regex compiles")
    };
}

#[derive(Debug)]
pub struct Variables(HashMap<String, String>);

impl Variables {
    pub fn new() -> Variables {
        Variables(HashMap::new())
    }
    pub fn expand(&self, src: &str) -> String {
        // TODO(tailhook) proper failure when no such var
        VAR_REGEX.replace_all(src, |caps: &Captures| {
            let name = caps.get(1).unwrap();
            match self.0.get(name.as_str()) {
                Some(x) => x.clone(),
                None => format!("<< unknown variable: {} >>", name.as_str()),
            }
        }).to_string()
    }
    pub fn add<A: AsRef<str>, B: AsRef<str>>(mut self, a: A, b: B)
        -> Variables
    {
        self.0.insert(a.as_ref().to_string(), b.as_ref().to_string());
        self
    }
    pub fn add_source(mut self, src: &apply::Source) -> Variables
    {
        // TODO(tailhook) proper failure when tmpfile can't be stringified
        use apply::Source::*;
        match *src {
            TmpFiles(ref templates) => {
                for (name, path) in templates {
                    self.0.insert(format!("files.{}", name),
                                  path.path().display().to_string());
                }
            }
        }
        self
    }
}
