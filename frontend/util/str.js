export function ends_with(x, value) {
    let dist = x.length - value.length
    return dist >= 0 && x.lastIndexOf(value) == dist
}

export function starts_with(x, value) {
    return x.length >= value.length && x.indexOf(value) == 0
}
