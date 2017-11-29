use std::process::Command;

use quire::validate as V;

use apply::{Task, Error, Action};
use apply::expand::Variables;

#[derive(Debug, Clone, Deserialize)]
pub struct Sh(String);

impl Sh {
    pub fn config() -> V::Scalar {
        V::Scalar::new()
    }
}

impl Action for Sh {
    fn execute(&self, mut task: Task, variables: Variables)
        -> Result<(), Error>
    {
        let mut cmd = Command::new("sh");
        cmd.arg("-exc");
        cmd.arg(variables.expand(&self.0));
        // TODO(tailhook) redirect output
        task.log(format_args!("Sh {:#?}\n", cmd));
        if !task.dry_run {
            try!(task.log.redirect_command(&mut cmd));
            cmd.status()
            .map_err(|e| {
                task.log.log(format_args!(
                    "Sh {:#?} failed to start: {}\n", cmd, e));
                Error::CantRun(
                    task.runner.to_string(), format!("{:#?}", cmd), e)
            }).and_then(|s| if s.success() { Ok(()) } else {
                task.log.log(format_args!("Sh {:#?}: {}\n", cmd, s));
                Err(Error::Command(
                    task.runner.to_string(), format!("{:#?}", cmd), s))
            })
        } else {
            Ok(())
        }
    }

}
