use std::process::Command;

use libc::geteuid;
use quire::validate as V;

use apply::{Task, Error, Action};
use apply::expand::Variables;

#[derive(Debug, Clone, Deserialize)]
pub struct RootCommand(Vec<String>);

impl RootCommand {
    pub fn config() -> V::Sequence<'static> {
        V::Sequence::new(V::Scalar::new())
        //.from_scalar(..)
    }
}

impl Action for RootCommand {
    fn execute(&self, mut task: Task, variables: Variables)
        -> Result<(), Error>
    {
        let uid = unsafe { geteuid() };
        let mut cmd = if uid != 0 {
            let mut cmd = Command::new("/usr/bin/sudo");
            cmd.arg("-n");
            cmd.arg(&variables.expand(&self.0[0]));
            cmd
        } else {
            Command::new(&self.0[0])
        };
        for arg in &self.0[1..] {
            cmd.arg(variables.expand(arg));
        }
        // TODO(tailhook) redirect output
        task.log(format_args!("RootCommand {:#?}\n", cmd));
        if !task.dry_run {
            try!(task.log.redirect_command(&mut cmd));
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

}
