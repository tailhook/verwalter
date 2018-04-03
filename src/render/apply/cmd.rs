use std::process::Command;

use quire::validate as V;

use apply::{Task, Error, Action};
use apply::expand::Variables;

#[derive(Debug, Clone, Deserialize)]
pub struct Cmd(Vec<String>);

impl Cmd {
    pub fn config() -> V::Sequence<'static> {
        V::Sequence::new(V::Scalar::new())
        //.from_scalar(..)
    }
}

impl Action for Cmd {
    fn execute(&self, task: &mut Task, variables: &Variables)
        -> Result<(), Error>
    {
        let mut cmd = Command::new(&self.0[0]);
        for arg in &self.0[1..] {
            cmd.arg(variables.expand(arg));
        }
        // TODO(tailhook) redirect output
        task.log(format_args!("Cmd {:#?}\n", cmd));
        if !task.dry_run {
            try!(task.log.redirect_command(&mut cmd));
            cmd.status()
            .map_err(|e| {
                task.log.log(format_args!(
                    "Cmd {:#?} failed to start: {}\n", cmd, e));
                format_err!("can't run {}: {:#?}: {}", task.runner, cmd, e)
            }).and_then(|s| if s.success() { Ok(()) } else {
                task.log.log(format_args!("Cmd {:#?}: {}\n", cmd, s));
                bail!("error running {}: {:#?}: {}", task.runner, cmd, s);
            })
        } else {
            Ok(())
        }
    }

}
