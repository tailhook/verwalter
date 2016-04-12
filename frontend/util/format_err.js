export function format_error_hint(err) {
    let text = ""
    for(var k in err) {
        text += `${k}: ${err[k]}\n`
    }
    return text
}

export function format_error_badge(err) {
    for(var k in err) {
        return k
    }
}
