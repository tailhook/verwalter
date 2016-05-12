pub fn path_component(x: &str) -> (&str, &str) {
    if x.len() == 0 {
        return ("", "");
    }
    match x[1..].find("/") {
        Some(n) => (&x[1..n+1], &x[n+1..]),
        None => (&x[1..], ""),
    }
}
