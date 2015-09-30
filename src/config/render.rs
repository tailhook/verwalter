use std::io::Read;
use std::fs::File;
use std::path::{Path};
use std::collections::HashMap;

use scan_dir;
use quire::validate as V;
use quire::sky::parse_config;
use handlebars::Handlebars;

use path_util::relative;
use render::{Renderer, RenderSet};
use super::TemplateError as Error;
use super::TemplateErrors;

// Configuration in .vw.yaml
#[derive(RustcDecodable, Debug)]
struct Config {
    render: HashMap<String, Renderer>,
}

fn config_validator<'x>() -> V::Structure<'x> {
    V::Structure::new()
    .member("render", V::Mapping::new(
        V::Scalar::new(),
        V::Structure::new()
        .member("source", V::Scalar::new())
        .member("apply", V::Enum::new().optional()
            .option("RootCommand",
                V::Sequence::new(V::Scalar::new())
                //.from_scalar(..)
            ))
        ))
}

pub fn read_config(path: &Path, base: &Path)
    -> Result<Vec<(String, Renderer)>, Error>
{
    debug!("Reading config {:?}", path);
    let piece: Config = try!(parse_config(&path,
        &config_validator(), Default::default())
        .map_err(|e| Error::Config(e, path.to_path_buf())));

    Ok(piece.render.into_iter()
    .map(|(name, r)| {
        (name,
         Renderer {
            // Normalize path to be relative to base path
            // rather than relative to current subdir
            source: relative(
                &path.parent().unwrap().join(r.source),
                base,
            ).unwrap().to_string_lossy().to_string(),
            apply: r.apply,
        })
    }).collect())
}

pub fn read_renderers(path: &Path) -> Result<RenderSet, TemplateErrors> {
    use super::TemplateError::{TemplateRead, TemplateParse};
    let mut errors: Vec<Error> = Vec::new();
    let mut render_set = RenderSet {
        items: HashMap::new(),
        handlebars: Handlebars::new(),
    };
    scan_dir::ScanDir::files().walk(path, |iter| {
        for (entry, fname) in iter {
            if fname.ends_with(".hbs") || fname.ends_with(".handlebars")
            {
                let epath = entry.path();
                debug!("Reading Handlebars template {:?}", epath);
                let mut buf = String::with_capacity(4096);
                let rpath = relative(&epath, &path).unwrap();
                File::open(&epath)
                .and_then(|mut x| x.read_to_string(&mut buf))
                .map_err(|e| errors.push(TemplateRead(e, epath.clone()))).ok()
                .and_then(|_| {
                    debug!("Adding template {:?}", epath);
                    render_set.handlebars.register_template_string(
                        &rpath.to_string_lossy(), buf)
                    .map_err(|e| errors.push(
                        TemplateParse(e, path.to_path_buf()))).ok()
                });
            } else if fname.ends_with(".vw.yaml") ||
                      fname.ends_with(".vw.yml")
            {
                let epath = entry.path();
                let rpath = relative(&epath, &path).unwrap();
                let spath = rpath.to_string_lossy();
                debug!("Reading render task {:?}", epath);
                read_config(&epath, &path)
                .map_err(|e| errors.push(e)).ok()
                .map(|v| render_set.items.extend(
                    v.into_iter().map(|(name, rnd)| {
                        (format!("{}:{}", spath, name), rnd)
                    })));
            } else {
                debug!("Ignored file {:?}", entry.path());
            }
        }
    })
    .map_err(|elst| errors.extend(elst.into_iter().map(From::from))).ok();
    if errors.len() > 0 {
        Err(TemplateErrors {
            errors: errors,
        })
    } else {
        Ok(render_set)
    }
}
