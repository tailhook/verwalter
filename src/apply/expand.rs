use std::collections::HashMap;

use regex::{Regex, Captures};
use apply;


lazy_static! {
    static ref VAR_REGEX: Regex = Regex::new(r"\{\{\s*(\w+)\s*\}\}").unwrap();
}

#[derive(Debug)]
pub struct Variables(HashMap<String, String>);

impl Variables {
    pub fn new() -> Variables {
        Variables(HashMap::new())
    }
    pub fn expand(&self, src: &str) -> String {
        // TODO(tailhook) proper failure when no such var
        VAR_REGEX.replace(src, |caps: &Captures| {
            let name = caps.at(1).unwrap();
            match self.0.get(name) {
                Some(x) => x.clone(),
                None => format!("<< unknown variable: {} >>", name).into(),
            }
        })
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
            TmpFile(ref x) => {
                self.0.insert("tmp_file".into(),
                              x.path().display().to_string());
            }
        }
        self
    }
}
