use std::path::PathBuf;
use std::time::Duration;
use std::sync::Arc;
use std::borrow::Cow;
use std::process::{Command, ExitStatus};
use std::collections::{BTreeMap};

use time::now_utc;
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
    pub use_sudo: bool,
    pub log_dir: PathBuf,
    pub config_dir: PathBuf,
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
                .group_by_lazy(|&(k, _)| k).into_iter()
                .map(|(k, mut vv)| (k.clone(), vv.next().unwrap().1.clone()))
                .collect()))
        }
    }).collect()
}

fn decode_render_error(s: ExitStatus) -> Cow<'static, str> {
    match s.code() {
        // Please, keep the docs in `doc/running/exit_codes` up to date
        Some(2) => "argparse error (should be a bug or version mismatch)",
        Some(3) => "argument validation error \
                    (should be a bug or version mismatch)",
        Some(4) => "no `template` key found => fix the scheduler",
        Some(5) => "version mismatch => restart the verwalter daemon",
        Some(10) => "error rendering templates",
        Some(20) => "error executing commands",
        Some(81) => "error when doing logging",
        Some(_) => {
            return format!("unknown code {}, please report a bug!", s).into();
        }
        None => {
            return format!("dead on signal: {}", s).into();
        }
    }.into()
}

fn apply_schedule(hash: &String, scheduler_result: &Json, settings: &Settings,
    debug_info: Arc<(Json, String)>, state: &SharedState)
{
    let id: String = thread_rng().gen_ascii_chars().take(24).collect();
    let mut index = Index::new(&settings.log_dir, settings.dry_run);
    let mut dlog = index.deployment(&id, true);
    dlog.string("schedule-hash", &hash);
    if debug_info.0 != Json::Null {
        dlog.gron("scheduler_input", &debug_info.0);
    }
    if debug_info.1 != "" {
        dlog.text("scheduler-debug", &debug_info.1);
    }
    dlog.gron("scheduler_result", scheduler_result);

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
    let string_schedule = format!("{}", scheduler_result);

    for (role_name, ref node_role_vars) in node_roles.iter() {
        let mut rlog = match dlog.role(&role_name, true) {
            Ok(l) => l,
            Err(e) => {
                error!("Can't create role log: {}", e);
                return;
            }
        };
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
        cur_vars.insert(String::from("deployment_id"),
            Json::String(id.clone()));
        cur_vars.insert(String::from("verwalter_version"),
            Json::String(concat!("v", env!("CARGO_PKG_VERSION")).into()));
        cur_vars.insert(String::from("timestamp"),
            Json::String(now_utc().rfc3339().to_string()));
        let vars = format!("{}", Json::Object(cur_vars));
        rlog.log(format_args!("Template variables: {}\n", vars));

        let mut cmd = if settings.use_sudo {
            let mut cmd = Command::new("sudo");
            cmd.arg("verwalter_render");
            cmd
        } else {
            Command::new("verwalter_render")
        };
        cmd.arg(&vars);
        cmd.arg("--log-dir");
        cmd.arg(&settings.log_dir);
        cmd.arg("--config-dir");
        cmd.arg(&settings.config_dir);
        cmd.arg("--schedule");
        cmd.arg(&string_schedule);
        if settings.dry_run {
            cmd.arg("--dry-run");
        }
        debug!("Running {:?}", cmd);
        match cmd.status() {
            Ok(x) if x.success() => {
                rlog.log(format_args!("Rendered successfully\n"));
                state.reset_role_failure(role_name);
            }
            Ok(status) => {
                state.mark_role_failure(role_name);
                rlog.log(format_args!(
                    "ERROR: Error rendering role. \
                    verwalter_render {}\n", status));
                rlog.log(format_args!(
                    "Decoded verwalter render failure: {}\n",
                    decode_render_error(status)));
            }
            Err(e) => {
                state.mark_role_failure(role_name);
                rlog.log(format_args!(
                    "ERROR: Error rendering role. \
                    Can't run verwalter_render: {}\n", e));
            }
        }
    }
    for err in dlog.done() {
        error!("Logging error: {}", err);
    }
}

pub fn run(state: SharedState, settings: Settings, mut alarm: Alarm) -> ! {
    let _guard = ExitOnReturn(93);
    let mut prev_schedule = String::new();
    if let Some(schedule) = state.stable_schedule() {
        let _alarm = alarm.after(Duration::from_secs(180));
        write_file(&settings.schedule_file, &*schedule)
            .map_err(|e| error!("Writing schedule failed: {:?}", e)).ok();
        apply_schedule(&schedule.hash, &schedule.data, &settings,
            state.scheduler_debug_info(), &state);
        prev_schedule = schedule.hash.clone();
    }
    loop {
        let schedule = state.wait_new_schedule(&prev_schedule);
        let _alarm = alarm.after(Duration::from_secs(180));
        write_file(&settings.schedule_file, &*schedule)
            .map_err(|e| error!("Writing schedule failed: {:?}", e)).ok();
        apply_schedule(&schedule.hash, &schedule.data, &settings,
            state.scheduler_debug_info(), &state);
        prev_schedule = schedule.hash.clone();
    }
}

#[cfg(test)]
mod tests {
    use rustc_serialize::json::Json;
    use super::merge_vars;

    fn parse_str(s: &str) -> Json {
        Json::from_str(s).unwrap()
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
