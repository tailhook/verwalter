extern crate rand;
extern crate argparse;
extern crate tempfile;
extern crate handlebars;
extern crate indexed_log;
extern crate rustc_serialize;
#[macro_use] extern crate quick_error;
#[macro_use] extern crate lazy_static;

#[macro_use] mod macros;
mod fs_util;
//mod render;
//mod apply;

use std::path::PathBuf;

use argparse::{ArgumentParser, Parse, FromCommandLine};
use rustc_serialize::json;

struct ParseJson(json::Json);

impl FromCommandLine for ParseJson {
    fn from_argument(s: &str) -> Result<ParseJson, String> {
        json::Json::from_str(s).map_err(|x| x.to_string()).map(ParseJson)
    }
}


fn main() {
    let mut role = String::from("");
    let mut templates = PathBuf::from("");
    let mut vars = ParseJson(json::Json::Null);
    {
        let mut ap = ArgumentParser::new();
        ap.set_description("Internal verwalter's utility to render it");
        ap.refer(&mut role).add_argument("role", Parse, "
            Name of the role to render
            ").required();
        ap.refer(&mut templates).add_argument("templates", Parse, "
            Templates subdirectory to parse
            ").required();
        ap.refer(&mut vars).add_argument("vars", Parse, "
            Variables to pass to renderer
            ").required();
        ap.parse_args_or_exit();
    }
    println!("Role: {:?}, templates: {:?}, vars: {:?}", role, templates,
        &vars.0);
    //let log =
    //match prepare_files(&role, &templates, &vars.0) {
    //}
}
