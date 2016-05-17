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
use std::process::exit;

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
    let id = match vars.0.find("deployment_id").and_then(|x| x.as_string()) {
        Some(x) => x,
        None => exit(3),
    };
    let template = match vars.0.find("template").and_then(|x| x.as_string()) {
        Some(x) => x,
        None => exit(4),
    };
    let mut dlog = log.deployment(&id, false);
    let mut rlog = dlog.role(&role, false);
    match render::render_role(role, template, &vars.0, &mut rlog) {
        Ok(actions) => {
            //apply_list(&role_name, actions, &mut rlog, settings.dry_run);
        }
        Err(e) => {
            rlog.log(format_args!(
                "ERROR: Can't render templates: {}\n", e));
            // TODO(tailhook) should we still check dlog.errors()
            exit(10);
        }
    }
    if dlog.errors().len() != 0 {
        exit(81);
    }
}
