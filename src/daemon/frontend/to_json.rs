use libcantal;
use serde_json::Value;


/// This is a trait similar to `rust_serialize::json::ToJson` but allows to
/// implement or own conversions (because of orphan rules)
pub trait ToJson {
    fn js(&self) -> Value;
}

impl ToJson for libcantal::Counter {
    fn js(&self) -> Value {
        Value::Number(self.get().into())
    }
}

impl ToJson for libcantal::Integer {
    fn js(&self) -> Value {
        Value::Number(self.get().into())
    }
}

impl<T: ToJson> ToJson for AsRef<T> {
    fn js(&self) -> Value {
        self.as_ref().js()
    }
}
