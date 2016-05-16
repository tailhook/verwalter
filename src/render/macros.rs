#[macro_export]
macro_rules! tuple_struct_decode {
    ($name:ident) => {
        impl ::rustc_serialize::Decodable for $name {
            fn decode<D: ::rustc_serialize::Decoder>(d: &mut D)
                -> Result<$name, D::Error>
            {
                ::rustc_serialize::Decodable::decode(d).map($name)
            }
        }
    }
}
