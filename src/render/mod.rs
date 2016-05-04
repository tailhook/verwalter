use std::io;
use std::io::Write;
use std::collections::HashMap;
use std::collections::BTreeMap;

use tempfile::NamedTempFile;
use handlebars::{Handlebars, RenderError};
use rustc_serialize::json::{Json, ToJson};

use config::{Version, Role};
use apply::{Source, Command};
use apply::log;


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
    pub apply: Option<Command>,
    pub commands: Vec<Command>,
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
            display("error rendering template, version {:?}, file {:?}: \
                {}\n    data: {:?}", version, file, err, data)
            description("template rendering error")
        }
        RoleMeta(msg: &'static str) {
            display("role metadata error: {}", msg)
            description("bad role meta data")
        }
        NodeRole(msg: &'static str) {
            display("error of role metadata for current node: {}", msg)
            description("bad role meta data on this node")
        }
    }
}

pub fn render_role(meta: &BTreeMap<String, Json>,
    node: &BTreeMap<String, Json>,
    role_name: &String, role: &Role, log: &mut log::Role)
    -> Result<Vec<(String, Vec<Command>, Source)>, Error>
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
        log.template(&render.source, &tmpfile.path(), &output);
        let mut cmds = render.commands.clone();
        if let Some(ref x) = render.apply {
            log.log(format_args!("`apply:` is deprecated use `commands: []`"));
            cmds.push(x.clone());
        }
        result.push((name.clone(), cmds, Source::TmpFile(tmpfile)));
    }
    Ok(result)
}
