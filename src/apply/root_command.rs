use apply::{Source, Error, log};

pub fn execute(cmd: Vec<String>, source: Source,
    log: &mut log::Action, dry_run: bool)
    -> Result<(), Error>
{
    log.log(format_args!("RootCommand {:?}\n", cmd));
    unimplemented!();
}
