use std::path::PathBuf;
use std::time::Duration;
use std::process::Command;
use std::collections::{BTreeMap};

use rand::{thread_rng, Rng};
use itertools::Itertools;
use rustc_serialize::json::{Json};
use indexed_log::Index;

use fs_util::write_file;
use shared::{SharedState};
use watchdog::{ExitOnReturn, Alarm};


pub struct Settings {
    pub hostname: String,
    pub dry_run: bool,
    pub log_dir: PathBuf,
    pub schedule_file: PathBuf,
}

fn merge_vars<'x, I, J>(iter: I) -> BTreeMap<String, Json>
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
    .group_by_lazy(|&(key, _)| key).into_iter()
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
                .map(|(k, v)| (k.clone(), v.clone())).collect()))
        }
    }).collect()
}

fn apply_schedule(hash: &String, scheduler_result: &Json, settings: &Settings)
{
    let id = thread_rng().gen_ascii_chars().take(24).collect();
    let mut index = Index::new(&settings.log_dir, settings.dry_run);
    let mut dlog = index.deployment(id);
    dlog.string("schedule-hash", &hash);
    dlog.json("scheduler_result", &scheduler_result);

    let empty = BTreeMap::new();
    let roles = scheduler_result.as_object()
        .and_then(|x| x.get("roles"))
        .and_then(|y| y.as_object())
        .unwrap_or_else(|| {
            dlog.log(format_args!(
                "Warning: Can't find `roles` key in schedule\n"));
            &empty
        });
    let vars = scheduler_result.as_object()
        .and_then(|x| x.get("vars"))
        .and_then(|x| x.as_object())
        .unwrap_or(&empty);
    let node = scheduler_result.as_object()
        .and_then(|x| x.get("nodes"))
        .and_then(|y| y.as_object())
        .and_then(|x| x.get(&settings.hostname))
        .and_then(|y| y.as_object())
        .unwrap_or_else(|| {
            dlog.log(format_args!(
                "Warning: Can't find `nodes[{}]` key in schedule\n",
                settings.hostname));
            &empty
        });
    let node_vars = node.get("vars")
        .and_then(|x| x.as_object())
        .unwrap_or(&empty);
    let node_roles = node.get("roles")
        .and_then(|x| x.as_object())
        .unwrap_or(&empty);
    let all_roles = roles.keys().merge(node_roles.keys()).dedup();

    for role_name in all_roles {
        let mut rlog = match dlog.role(&role_name) {
            Ok(l) => l,
            Err(e) => {
                error!("Can't create role log: {}", e);
                return;
            }
        };
        let node_role_vars = node_roles.get(role_name)
            .and_then(|x| x.as_object())
            .unwrap_or(&empty);
        let role_vars = roles.get(role_name)
            .and_then(|x| x.as_object())
            .unwrap_or(&empty);
        let cur_vars = merge_vars(vec![
            node_role_vars.iter(),
            node_vars.iter(),
            role_vars.iter(),
            vars.iter(),
        ].into_iter());

        let template = match cur_vars.get("template")
            .and_then(|x| x.as_string())
        {
            Some(t) => t.to_string(),
            None => {
                rlog.log(format_args!(
                    "ERROR: Error rendering role. \
                    No `template` key found.\n"));
                continue;
            }
        };

        let mut cmd = Command::new("verwalter-render");
        cmd.arg(role_name);
        cmd.arg(template);
        cmd.arg(format!("{}", Json::Object(cur_vars)));
        debug!("Running {:?}", cmd);
        match cmd.status() {
            Ok(x) if x.success() => {
                rlog.log(format_args!("Rendered successfully"));
            }
            Ok(s) => {
                rlog.log(format_args!(
                    "ERROR: Error rendering role. \
                    verwalter-render {}\n", s));
            }
            Err(e) => {
                rlog.log(format_args!(
                    "ERROR: Error rendering role. \
                    Can't run verwalter-render: {}\n", e));
            }
        }
    }
}

pub fn run(state: SharedState, settings: Settings, mut alarm: Alarm) -> ! {
    let _guard = ExitOnReturn(93);
    let mut prev_schedule = String::new();
    if let Some(schedule) = state.stable_schedule() {
        let _alarm = alarm.after(Duration::from_secs(180));
        write_file(&settings.schedule_file, &*schedule)
            .map(|e| error!("Writing schedule failed: {:?}", e)).ok();
        apply_schedule(&schedule.hash, &schedule.data, &settings);
        prev_schedule = schedule.hash.clone();
    }
    loop {
        let schedule = state.wait_new_schedule(&prev_schedule);
        let _alarm = alarm.after(Duration::from_secs(180));
        write_file(&settings.schedule_file, &*schedule)
            .map(|e| error!("Writing schedule failed: {:?}", e)).ok();
        apply_schedule(&schedule.hash, &schedule.data, &settings);
        prev_schedule = schedule.hash.clone();
    }
}
