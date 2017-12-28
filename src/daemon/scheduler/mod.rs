use lua::{ThreadStatus, Type};

mod luatic;
mod execute;
mod state;
pub mod main;  // pub for making counters visible

pub use self::state::{Schedule, ScheduleId, from_json};
pub use self::main::{main as run, Settings, SchedulerInput};


quick_error! {
    #[derive(Debug)]
    pub enum Error {
        Lua(err: ThreadStatus, msg: String) {
            display("running lua script {:?}: {}", err, msg)
            description("error running scheduler")
        }
        FunctionNotFound(name: &'static str, typ: Type) {
            display("Main expected to export {:?} expected {:?} found",
                name, typ)
            description("scheduler function not found")
        }
        /*
        WrongValue(val: AnyLuaValue) {
            display("script returned non-string value: {:?}", val)
        }
        */
        UnexpectedYield {
            description("scheduler function should not yield")
        }
        Conversion {
            description("Scheduler returned unconvertible value")
        }
    }
}
