export function format_error_hint(status) {
    let text = ""
    for(var k in status.errors) {
        text += `${k}: ${status.errors[k]}\n`
    }
    for(var k of status.failed_roles) {
        text += `role ${k} failed to render\n`
    }
    return text
}

export function format_error_badge(status) {
    for(let k in status.errors) {
        return k
    }
    for(let k of status.failed_roles) {
        return 'role:' + k
    }
    return ""
}
