use std::io;
use std::io::Write;
use std::collections::HashMap;
use std::collections::BTreeMap;

use tempfile::NamedTempFile;
use handlebars::{Handlebars, RenderError};
use rustc_serialize::json::{Json, ToJson};

use config::{Config, Version, Role};
use apply::{Source, ApplyTask, Action};


pub struct RenderSet {
    pub items: HashMap<String, Renderer>,
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
    pub apply: Action,
}

quick_error! {
    #[derive(Debug)]
    pub enum Error {
        Skip {
            description("role is skipped on this node")
        }
        Io(err: io::Error) {
            from() cause(err)
            display("I/O error: {}", err)
            description("I/O error")
        }
        NoTemplates(version: String) {
            display("no templates version {:?} found", version)
            description("no templates for role (and version)")
        }
        BadTemplates(version: String, errors: Vec<String>) {
            display("bad templates, version {:?}, errors: {:?}",
                version, errors)
            description("couldn't parse this version of templates")
        }
        Render(err: RenderError, version: String, file: String,
               data: Json) {
            display("error rendering template, version {:?}, file {:?}, \
                data: {:?} -- {}", version, file, data, err)
            description("template rendering error")
        }
        RoleMeta(msg: &'static str) {
            display("{}", msg)
            description("bad role meta data")
        }
        NodeRole(msg: &'static str) {
            display("{}", msg)
            description("bad role meta data on this node")
        }
    }
}
quick_error!{
    #[derive(Debug)]
    pub enum SchedulerDataError {
        NoRoleMeta {
            description(r#"No key "role_metadata" or is not a dict"#)
        }
        MissingHost {
            description("The hostname is missing in scheduler data")
        }
    }
}

fn render_role(meta: &BTreeMap<String, Json>, node: &BTreeMap<String, Json>,
    role_name: &String, role: &Role, print: bool)
    -> Result<Vec<(String, Action, Source)>, Error>
{
    let role_meta = match meta.get(role_name) {
        Some(&Json::Object(ref ob)) => ob,
        Some(_) => {
            return Err(Error::RoleMeta("not an object"));
        }
        None => {
            debug!("No role in role metadata {:?}", role_name);
            // TODO(tailhook) is this Skip or real error?
            return Err(Error::Skip);
        }
    };
    let tpl_ver = match role_meta.get("templates") {
        Some(&Json::String(ref val)) => Version(val.to_string()),
        Some(_) => {
            return Err(Error::RoleMeta(r#""templates" is not a string"#));
        }
        None => {
            return Err(Error::RoleMeta(r#"no "templates" is role metadata"#));
        }
    };
    let rnd = match role.renderers.get(&tpl_ver) {
        Some(&Ok(ref rnd)) => rnd,
        Some(&Err(ref e)) => {
            return Err(Error::BadTemplates(tpl_ver.0.clone(),
                e.errors.iter().map(|x| x.to_string()).collect()));
        }
        None => {
            return Err(Error::NoTemplates(tpl_ver.0.clone()));
        }
    };
    let node_role = match node.get(role_name) {
        Some(&Json::Object(ref ob)) => ob,
        Some(_) => {
            return Err(Error::NodeRole("not an object"));
        }
        None => {
            debug!("No role {:?} in the node", role_name);
            return Err(Error::Skip);
        }
    };
    let mut result = Vec::new();
    for (name, render) in &rnd.items {
        let mut tmpfile = try!(NamedTempFile::new());
        let data = Json::Object(vec![
            ("verwalter_version".to_string(),
                concat!("v", env!("CARGO_PKG_VERSION")).to_json()),
            ("role".to_string(), Json::Object(role_meta.clone())),
            ("node".to_string(), Json::Object(node_role.clone())),
        ].into_iter().collect());

        let output = try!(rnd.handlebars.render(&render.source, &data)
            .map_err(|e| Error::Render(e,
                tpl_ver.0.clone(), render.source.clone(), data)));
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
        result.push((name.clone(), render.apply.clone(),
                     Source::TmpFile(tmpfile)));
    }
    Ok(result)
}

pub fn render_all<'x>(cfg: &'x Config, data: Json,
    hostname: String, print: bool)
    -> Result<ApplyTask, SchedulerDataError>
{
    let meta = data.as_object()
        .and_then(|x| x.get("role_metadata"))
        .and_then(|y| y.as_object());
    let meta = try!(meta.ok_or(SchedulerDataError::NoRoleMeta));
    let node = data.as_object()
        .and_then(|x| x.get("nodes"))
        .and_then(|y| y.as_object())
        .and_then(|x| x.get(&hostname))
        .and_then(|y| y.as_object());
    let node = try!(node.ok_or(SchedulerDataError::MissingHost));
    Ok(cfg.roles.iter().map(|(role_name, role)| {
        (role_name.clone(), render_role(meta, node, role_name, role, print))
    }).collect())
}
