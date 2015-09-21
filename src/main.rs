extern crate walker;
extern crate argparse;
extern crate rumblebars;
extern crate env_logger;
extern crate quire;
extern crate rustc_serialize;
#[macro_use] extern crate quick_error;
#[macro_use] extern crate log;

use std::path::PathBuf;
use std::process::exit;

mod path_util;
mod config;

use argparse::{ArgumentParser, Parse};


fn main() {
    env_logger::init().unwrap();
    let mut config_dir = PathBuf::from("/etc/verwalter");
    {
        let mut ap = ArgumentParser::new();
        ap.refer(&mut config_dir)
            .add_option(&["-D", "--config-dir"], Parse,
                "Directory of configuration files");
        ap.parse_args_or_exit();
    }
    let configs = match config::read_configs(&config_dir) {
        Ok(configs) => configs,
        Err(e) => {
            error!("Initial configuration load failed: {}", e);
            exit(2);
        }
    };
    debug!("Configuration read with, templates: {}, renderers: {}",
        configs.templates.len(), configs.renderers.len());

}
