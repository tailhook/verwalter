export function from_ms(ms) {
    let date = new Date()
    date.setTime(ms)
    return date
}

export function till_now_ms(dt) {
    let ms = new Date() - dt.getTime();
    return ms
}

function _two(n) {
    if(n < 10) {
        return '0' + n;
    }
    return '' + n;
}

export function format_datetime(dt) {
    return ( `${dt.getFullYear()}-${_two(dt.getMonth()+1)}-${_two(dt.getDate())}`
           + ` ${_two(dt.getHours())}:${_two(dt.getMinutes())}`
           + `:${_two(dt.getSeconds())}`)
}

export function format_date(dt) {
    return `${dt.getFullYear()}-${_two(dt.getMonth()+1)}-${_two(dt.getDate())}`
}

export function format_time(dt) {
    return `${_two(dt.getHours())}:${_two(dt.getMinutes())}`
}

export function format_uptime(ms) {
    if(ms < 1000) {
        return "∅";
    } else if(ms < 90000) {
        return `${(ms/1000)|0}s`
    } else if(ms < 5400000) {
        return `${(ms/60000)|0}m${((ms/1000) % 60)|0}s`
    } else if(ms < 86400000) {
        return `${(ms/3600000)|0}h${((ms/60000) % 60)|0}m`
    } else {
        return `${(ms/86400000)|0}d${((ms/3600000) % 24)|0}h`
    }
}

export function format_diff(ms) {
    if(ms < 1000) {
        return `${ms}ms`;
    } else if(ms < 90000) {
        return `${(ms/1000)|0}s`
    } else if(ms < 5400000) {
        return `${(ms/60000)|0}m${((ms/1000) % 60)|0}s`
    } else if(ms < 86400000) {
        return `${(ms/3600000)|0}h${((ms/60000) % 60)|0}m`
    } else {
        return `${(ms/86400000)|0}d${((ms/3600000) % 24)|0}h`
    }
}

export function short_time(dt) {
    let delta = till_now_ms(dt);
    if(delta < 2000) {
        return 'just now'
    }
    if(delta < 60000) {
        return (delta/1000).toFixed(0) + ' seconds ago'
    }
    if(delta < 43200000) {
        return format_time(dt);
    }
    return format_date(dt);
}
