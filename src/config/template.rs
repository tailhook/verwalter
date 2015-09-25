use std::fmt;
use std::io;
use std::io::Write;
use std::rc::Rc;
use std::path::PathBuf;
use std::collections::HashMap;
use std::os::raw::c_long;
use std::os::unix::raw::time_t;

use handlebars;
use rustc_serialize::json::Json;

pub type Template = Rc<Wrapper>;

struct Wrapper {
    mtime: (time_t, c_long),
    filename: PathBuf,
    implementation: handlebars::Template, // TODO(tailhook) will be enum later
}

impl fmt::Debug for Wrapper {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        write!(fmt, "Template({:?})", self.filename)
    }
}

pub struct Cache {
    items: HashMap<PathBuf, Template>,
}

quick_error! {
    #[derive(Debug)]
    pub enum RenderError {
        Io(err: io::Error) {
            from() cause(err)
            display("I/O error: {}", err)
        }
    }
}

impl Cache {
    pub fn new() -> Cache {
        Cache {
            items: HashMap::new(),
        }
    }
}

impl Wrapper {
    pub fn render(&self, data: &Json, out: &mut Write)
        -> Result<(), RenderError>
    {
        //let ectx = handlebars::EvalContext::new();
        //try!(handlebars::eval(&self.implementation, data, out, &ectx));
        Ok(())
    }
}
