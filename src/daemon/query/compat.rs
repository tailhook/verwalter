use std::collections::BTreeMap;
use std::sync::Arc;
use std::time::SystemTime;

use humantime::format_rfc3339;
use failure::{Error, err_msg};
use itertools::Itertools;
use query::Settings;
use serde_json::{Value as Json, Map};
use scheduler::{Schedule};

use query::{RolesResult, QueryData};


pub struct Responder {
    schedule: Arc<Schedule>,
    hostname: String,
}

impl Responder {
    pub fn new(schedule: &Arc<Schedule>, settings: &Settings) -> Responder {
        Responder {
            schedule: schedule.clone(),
            hostname: settings.hostname.clone(),
        }
    }
    pub fn render_roles(&self, _id: &str, _prev: Option<&Schedule>)
        -> Result<RolesResult, Error>
    {
        let empty = Map::new();
        let roles = self.schedule.data.as_object()
            .and_then(|x| x.get("roles"))
            .and_then(|y| y.as_object())
            .unwrap_or_else(|| {
                info!("Can't find `roles` key in schedule\n");
                &empty
            });
        let vars = self.schedule.data.as_object()
            .and_then(|x| x.get("vars"))
            .and_then(|x| x.as_object())
            .unwrap_or(&empty);
        let node = self.schedule.data.as_object()
            .and_then(|x| x.get("nodes"))
            .and_then(|y| y.as_object())
            .and_then(|x| x.get(&self.hostname))
            .and_then(|y| y.as_object())
            .unwrap_or_else(|| {
                warn!("Can't find `nodes[{}]` key in schedule\n",
                    self.hostname);
                &empty
            });
        let node_vars = node.get("vars")
            .and_then(|x| x.as_object())
            .unwrap_or(&empty);
        let node_roles = node.get("roles")
            .and_then(|x| x.as_object())
            .unwrap_or(&empty);

        let mut to_render = BTreeMap::new();
        for (role_name, ref node_role_vars) in node_roles.iter() {
            let node_role_vars = node_role_vars.as_object().unwrap_or(&empty);
            let role_vars = roles.get(role_name)
                .and_then(|x| x.as_object())
                .unwrap_or(&empty);
            let mut cur_vars = merge_vars(vec![
                node_role_vars.iter(),
                node_vars.iter(),
                role_vars.iter(),
                vars.iter(),
            ].into_iter());
            cur_vars.insert(String::from("role"),
                Json::String(role_name.clone()));
            if !cur_vars.contains_key("node") {
                cur_vars.insert(String::from("node"),
                    Json::String(self.hostname.clone()));
            }
            cur_vars.insert(String::from("timestamp"),
                Json::String(format_rfc3339(SystemTime::now()).to_string()));
            to_render.insert(role_name.clone(), Json::Object(cur_vars));
        }
        let all_roles = roles.keys().cloned()
            .chain(node_roles.values()
                .flat_map(|x| x.as_object())
                .flat_map(|x| x.keys().cloned()))
            .collect();
        Ok(RolesResult {
            all_roles,
            to_render,
        })
    }
    pub fn schedule(&self) -> Arc<Schedule> {
        self.schedule.clone()
    }
    pub fn query(&self, _data: QueryData) -> Result<Json, Error> {
        return Err(err_msg("no query interface supported"));
    }
}


fn merge_vars<'x, I, J>(iter: I) -> Map<String, Json>
    where I: Iterator<Item=J>, J: Iterator<Item=(&'x String, &'x Json)>
{

    struct Wrapper<'a>((&'a String, &'a Json));
    impl<'a> PartialOrd for Wrapper<'a> {
        fn partial_cmp(&self, other: &Wrapper)
            -> Option<::std::cmp::Ordering>
        {
            (self.0).0.partial_cmp((other.0).0)
        }
    }
    impl<'a> PartialEq for Wrapper<'a> {
        fn eq(&self, other: &Wrapper) -> bool {
            (self.0).0.eq((other.0).0)
        }
    }
    impl<'a> Eq for Wrapper<'a> {};
    impl<'a> Ord for Wrapper<'a> {
        fn cmp(&self, other: &Wrapper) -> ::std::cmp::Ordering {
            (self.0).0.cmp((other.0).0)
        }
    };

    iter.map(|x| x.map(Wrapper)).kmerge().map(|x| x.0)
    .group_by(|&(key, _)| key).into_iter()
    .map(|(key, vals)| {
        let x = vals.map(|(_, v)| v).coalesce(|x, y| {
            match (x, y) {
                // If both are objects, they are candidates to merge
                (x@&Json::Object(_), y@&Json::Object(_)) => Err((x, y)),
                // If first is not an object we use it value
                // (it overrides/replaces following)
                // If second is not an object, we just skip it, because we
                // can't merge it anyway
                (x, _) => Ok(x),
            }
        }).collect::<Vec<_>>();
        if x.len() == 1 {
            (key.clone(), x[0].clone())
        } else {
            (key.clone(), Json::Object(
                x.iter()
                .map(|x| x.as_object().unwrap().iter())
                .map(|x| x.map(Wrapper)).kmerge().map(|x| x.0)
                .group_by(|&(k, _)| k).into_iter()
                .map(|(k, mut vv)| (k.clone(), vv.next().unwrap().1.clone()))
                .collect()))
        }
    }).collect()
}

#[cfg(test)]
mod tests {
    use serde_json::Value as Json;
    use serde_json::from_str;
    use super::merge_vars;

    fn parse_str(s: &str) -> Json {
        from_str(s).unwrap()
    }

    #[test]
    fn test_merge_simple() {
        let a = parse_str(r#"{"lamp": "blue", "table": "green"}"#);
        let b = parse_str(r#"{"lamp": "yellow", "chair": "black"}"#);
        assert_eq!(Json::Object(merge_vars(vec![
            a.as_object().unwrap().iter(),
            b.as_object().unwrap().iter(),
            ].into_iter())), parse_str(
            r#"{"lamp": "yellow", "table": "green", "chair": "black"}"#));
    }

    #[test]
    fn test_merge_nested() {
        let a = parse_str(r#"{"a": {"lamp": "blue", "table": "green"}}"#);
        let b = parse_str(r#"{"a": {"lamp": "yellow", "chair": "black"}}"#);
        assert_eq!(Json::Object(merge_vars(vec![
            a.as_object().unwrap().iter(),
            b.as_object().unwrap().iter(),
            ].into_iter())), parse_str(
        r#"{"a": {"lamp": "yellow", "table": "green", "chair": "black"}}"#));
    }
}
