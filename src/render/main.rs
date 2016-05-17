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

use argparse::{ArgumentParser, Parse, StoreTrue, FromCommandLine};
use indexed_log::Index;
use rustc_serialize::json;

struct ParseJson(json::Json);

impl FromCommandLine for ParseJson {
    fn from_argument(s: &str) -> Result<ParseJson, String> {
        json::Json::from_str(s).map_err(|x| x.to_string()).map(ParseJson)
    }
}


fn main() {
    let mut role = String::from("");
    let mut vars = ParseJson(json::Json::Null);
    let mut log_dir = PathBuf::from("");
    let mut dry_run = false;
    {
        let mut ap = ArgumentParser::new();
        ap.set_description("Internal verwalter's utility to render it");
        ap.refer(&mut role).add_argument("role", Parse, "
            Name of the role to render
            ").required();
        ap.refer(&mut vars).add_argument("vars", Parse, "
            Variables to pass to renderer
            ").required();
        ap.refer(&mut dry_run).add_option(&["--dry-run"], StoreTrue, "
            Don't run commands just show the templates and command-lines.
            ");
        ap.refer(&mut log_dir).add_option(&["--log-dir"], Parse, "
            Log directory");
        ap.parse_args_or_exit();
    }
    let mut log = Index::new(&log_dir, dry_run);
    let id = vars.0.find("deployment_id").and_then(|x| x.as_string())
        .expect("The `deployment_id` must be present");
    let mut dlog = log.deployment(&id, false);
    let mut role = dlog.role(&role, false);
}
