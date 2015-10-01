use std::process::Command;

use libc::geteuid;

use apply::{Task, Error};


pub fn execute(args: Vec<String>, mut task: Task) -> Result<(), Error>
{
    let uid = unsafe { geteuid() };
    let mut cmd = if uid != 0 {
        let mut cmd = Command::new("/usr/bin/sudo");
        cmd.arg("-n");
        cmd.arg(&args[0]);
        cmd
    } else {
        Command::new(&args[0])
    };
    cmd.args(&args[1..]);
    task.log(format_args!("RootCommand {:#?}\n", cmd));
    if !task.dry_run {
        cmd.status()
        .map_err(|e| {
            task.log.log(format_args!(
                "RootCommand {:#?} failed to start: {}\n", cmd, e));
            Error::CantRun(
                task.runner.to_string(), format!("{:#?}", cmd), e)
        }).and_then(|s| if s.success() { Ok(()) } else {
            task.log.log(format_args!("RootCommand {:#?}: {}\n", cmd, s));
            Err(Error::Command(
                task.runner.to_string(), format!("{:#?}", cmd), s))
        })
    } else {
        Ok(())
    }
}
