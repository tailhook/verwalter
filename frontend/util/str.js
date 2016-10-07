export function ends_with(x, value) {
    let dist = x.length - value.length
    return dist >= 0 && x.lastIndexOf(value) == dist
}
