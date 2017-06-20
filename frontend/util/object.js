export function entries(x) {
    return Object.keys(x).map(k => [k, x[k]])
}

export function entries_sorted(x) {
    let keys = Object.keys(x)
    keys.sort()
    return keys.map(k => [k, x[k]])
}

export function keys(x) {
    return Object.keys(x)
}

export function repr(x) {
    return JSON.stringify(x)
}

export function pretty(x) {
    return JSON.stringify(x, null, 2)
}

export function is_string(x) {
    return typeof x == 'string'
}

export function reversed(x) {
    let r = x.concat();
    r.reverse();
    return r
}

export function pretty_json(x) {
    return JSON.stringify(x, null, 2)
}

export function* enumerate(lst) {
    for(var i = 0; i < lst.length; ++i) {
        yield [i, lst[i]]
    }
}
