use std::io;
use std::fmt::{Arguments, Debug};
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;
use std::process::Command;
use std::process::ExitStatus;
use std::collections::HashMap;

use rand::{thread_rng, Rng};
use tempfile::NamedTempFile;
use rustc_serialize::{Decodable, Decoder};
use rustc_serialize::json::{Json, ToJson};
use indexed_log::Index;

use fs_util::write_file;
use shared::{Id, Peer, SharedState};
use watchdog::{ExitOnReturn, Alarm};


pub struct Settings {
    pub hostname: String,
    pub dry_run: bool,
    pub log_dir: PathBuf,
    pub schedule_file: PathBuf,
}

fn apply_schedule(hash: &String, scheduler_result: &Json, settings: &Settings)
{
    let id = thread_rng().gen_ascii_chars().take(24).collect();
    let mut index = Index::new(&settings.log_dir, settings.dry_run);
    let mut dlog = index.deployment(id);
    dlog.string("schedule-hash", &hash);
    dlog.json("scheduler_result", &scheduler_result);

    let meta = scheduler_result.as_object()
        .and_then(|x| x.get("role_metadata"))
        .and_then(|y| y.as_object());
    let meta = match meta {
        Some(meta) => meta,
        None => {
            dlog.log(format_args!(
                "FATAL ERROR: Can't find `role_metadata` key in schedule\n"));
            error!("Can't find `role_metadata` key in schedule");
            return
        }
    };
    let node = scheduler_result.as_object()
        .and_then(|x| x.get("nodes"))
        .and_then(|y| y.as_object())
        .and_then(|x| x.get(&settings.hostname))
        .and_then(|y| y.as_object());
    let node = match node {
        Some(node) => node,
        None => {
            dlog.log(format_args!(
                "FATAL ERROR: Can't find node {:?} in `nodes` \
                    key in schedule\n",
                &settings.hostname));
            error!("Can't find node {:?} in `nodes` key in schedule",
                &settings.hostname);
            return;
        }
    };

    for (role_name, info) in node.iter() {
        let mut rlog = match dlog.role(&role_name) {
            Ok(l) => l,
            Err(e) => {
                error!("Can't create role log: {}", e);
                return;
            }
        };
        let mut cmd = Command::new("verwalter-render");
        cmd.arg(role_name);
        // TODO cmd.arg(template_name);
        // TODO cmd.arg(merged_data);
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
