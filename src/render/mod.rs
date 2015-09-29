use std::io;
use std::io::Write;

use tempfile::NamedTempFile;
use handlebars::{Handlebars, RenderError};
use rustc_serialize::json::{Json, ToJson};

use config::{Config, Version};


pub struct RenderSet {
    pub items: Vec<Renderer>,
    pub handlebars: Handlebars,
}

impl ::std::fmt::Debug for RenderSet {
    fn fmt(&self, fmt: &mut ::std::fmt::Formatter) -> ::std::fmt::Result {
        fmt.debug_map()
            .entry(&"renderers", &self.items)
            .entry(&"handlebars", &"Handlebars")
            .finish()
    }
}

#[derive(RustcDecodable, Debug)]
pub struct Renderer {
    pub source: String,
    pub apply: Command,
}

#[derive(RustcDecodable, Debug, Clone)]
pub enum Command {
    RootCommand(Vec<String>),
}

quick_error! {
    #[derive(Debug)]
    pub enum Error {
        Io(err: io::Error) {
            from() cause(err)
            display("I/O error: {}", err)
            description("I/O error")
        }
        SchedulerData(msg: &'static str) {
            display("Bad scheduler data {}", msg)
            description("bad scheduler data")
        }
        NoTemplates(role: String, version: String) {
            display("No templates for role {:?}, template version {:?}",
                role, version)
            description("No templates for role (and version)")
        }
        BadTemplates(role: String, version: String, errors: Vec<String>) {
            display("Bad templates for role {:?}, template version {:?},\
                errors: {:?}",
                role, version, errors)
            description("couldn't parse this version of templates")
        }
        Render(err: RenderError, role: String, version: String, file: String,
               data: Json) {
            display("Error rendering template of {:?} ver {:?} file {:?}, \
                data: {:?} -- {}", role, version, file, data, err)
            description("template rendering error")
        }
        RoleMeta(role: String, msg: &'static str) {
            display("Metadata for role {:?} {}", role, msg)
            description("bad role meta data")
        }
        NodeRole(role: String, node: String, msg: &'static str) {
            display("Metadata for role {:?} on this node {}", role, msg)
            description("bad role meta data on this node")
        }
        NodeNotFound(node: String) {
            display("node {:?} not found in scheduler metadata", node)
            description("node not found in scheduler metadata")
        }
    }
}

pub fn render_all<'x>(cfg: &'x Config, data: Json,
    hostname: String, print: bool)
    -> Result<Vec<(NamedTempFile, Command)>, Error>
{
    let mut result = Vec::new();
    let meta = data.as_object()
        .and_then(|x| x.get("role_metadata"))
        .and_then(|y| y.as_object());
    let meta = match meta {
        Some(x) => x,
        None => {
            return Err(Error::SchedulerData(
                r#"No key "role_metadata" or is not a dict"#));
        }
    };
    let node = data.as_object()
        .and_then(|x| x.get("nodes"))
        .and_then(|y| y.as_object())
        .and_then(|x| x.get(&hostname))
        .and_then(|y| y.as_object());
    let node = match node {
        Some(x) => x,
        None => {
            return Err(Error::NodeNotFound(hostname.clone()));
        }
    };
    for (name, role) in &cfg.roles {
        let role_meta = match meta.get(name) {
            Some(&Json::Object(ref ob)) => ob,
            Some(_) => {
                return Err(Error::RoleMeta(name.clone(), "not an object"));
            }
            None => {
                debug!("Skipping role {:?}", name);
                continue;
            }
        };
        let tpl_ver = match role_meta.get("templates") {
            Some(&Json::String(ref val)) => Version(val.to_string()),
            Some(_) => {
                return Err(Error::RoleMeta(name.clone(),
                    r#""templates" is not a string"#));
            }
            None => {
                return Err(Error::RoleMeta(name.clone(),
                    r#"no "templates" is role metadata"#));
            }
        };
        let rnd = match role.renderers.get(&tpl_ver) {
            Some(&Ok(ref rnd)) => rnd,
            Some(&Err(ref e)) => {
                return Err(Error::BadTemplates(name.clone(), tpl_ver.0.clone(),
                    e.errors.iter().map(|x| x.to_string()).collect()));
            }
            None => {
                return Err(Error::NoTemplates(name.clone(), tpl_ver.0.clone()));
            }
        };
        let node_role = match node.get(name) {
            Some(&Json::Object(ref ob)) => ob,
            Some(_) => {
                return Err(Error::NodeRole(name.clone(), hostname.clone(),
                                           "not an object"));
            }
            None => {
                debug!("No role {:?} at node {:?}", name, hostname);
                continue;
            }
        };
        for render in &rnd.items {
            let mut tmpfile = try!(NamedTempFile::new());
            let data = Json::Object(vec![
                ("verwalter_version".to_string(),
                    concat!("v", env!("CARGO_PKG_VERSION")).to_json()),
                ("role".to_string(), Json::Object(role_meta.clone())),
                ("node".to_string(), Json::Object(node_role.clone())),
            ].into_iter().collect());

            let output = try!(rnd.handlebars.render(&render.source, &data)
                .map_err(|e| Error::Render(e,
                    name.clone(), tpl_ver.0.clone(), render.source.clone(),
                    data)));
            try!(tmpfile.write_all(output.as_bytes()));
            debug!("Rendered {:?} into {} bytes at {:?}",
                &render.source, output.as_bytes().len(), tmpfile.path());
            if print {
                println!("----- [ {:?} -> {:?} ] -----",
                    render.source, tmpfile.path());
                print!("{}", output);
                println!("----- End of [ {:?} -> {:?} ] -----",
                    render.source, tmpfile.path());
            }
            result.push((tmpfile, render.apply.clone()));
        }
    }
    Ok(result)
}
