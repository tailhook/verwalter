import {UPDATE_REQUEST, DATA, ERROR} from '../middleware/request'

export var fetch_indexes = {
    type: UPDATE_REQUEST,
    url: "/v1/log/index/latest",
    response_type: 'text',
    headers: {'Range': 'bytes=-1048576'},
    decoder: x => x,
}

export var role_messages = role => (state={}, action) => {
    switch(action.type) {
        case DATA:
            let items = parse_log(action.data, action.req)
            let deploys = new Map()
            let myitems = []
            // TODO(tailhook) use computed `record.role` produced
            // by `parse_log`
            let in_role = false
            for(let record of items) {
                let [time, dep, {variant, fields: [ident]}, val] = record
                if(ident != role && !in_role) {
                    continue
                }
                if(!deploys.get(dep)) {
                    deploys.set(dep, {externals: new Map()})
                }
                switch(val) {
                    case 'RoleStart':
                        deploys.get(dep).start = record;
                        in_role = true;
                        break;
                    case 'RoleFinish':
                        deploys.get(dep).finish = record;
                        in_role = false;
                        break;
                    case 'ExternalLog':
                        deploys.get(dep).externals.set(ident, record);
                        break;
                }
            }
            return {deploys: deploys}
    }
    return {}
}

function parse_log(data, req) {
    let lines = data.split('\n');
    // last item is always either empty or broken
    lines.pop();
    let rng = req.getResponseHeader("Content-Range")
    if(rng.substr(0, 8) != 'bytes 0-') {
        // if not a start of file, first item is broken too
        lines.shift();
    }
    let items = []
    let in_role = null
    for(var line of lines) {
        let record
        try {
            record = JSON.parse(line);
        } catch(e) {
            console.log("Bad index line", line, e);
            continue
        }
        let in_role_next = in_role
        let [time, dep, {variant, fields: [ident]}, val] = record
        switch(val) {
            case 'RoleStart':
                in_role = in_role_next = ident;
                break;
            case 'RoleFinish':
                in_role_next = null;
                break;
        }
        record.role = in_role;
        items.push(record);
        in_role = in_role_next;
    }
    return items
}

export function index(state={}, action) {
    switch(action.type) {
        case DATA:
            let items = parse_log(action.data, action.req)
            state = {...state, 'items': items};
            break;
    }
    return state;
}

export function log(state={}, action) {
    switch(action.type) {
        case DATA:
            state = {...state, 'text': action.data};
            break;
        case ERROR:
            state = {...state, 'text': action.error};
            break;
        case "show_mark":
            state = {...state, 'fetching': action.mark};
            break;
    }
    return state;
}

export function show_mark(mark) {
    return {
        type: "show_mark",
        mark: mark,
    }
}

export function view_from(mark) {
    let url;
    let off;
    if(mark.variant == 'Global') {
        url = "/v1/log/global/log." + mark.fields[0] + ".txt";
        off = mark.fields[1];
    } else if(mark.variant == 'Changes') {
        url = "/v1/log/changes/log." + mark.fields[0] + ".txt";
        off = mark.fields[1];
    } else if(mark.variant == 'Role') {
        url = "/v1/log/role/" + mark.fields[0] +
                "/log." + mark.fields[1] + ".txt";
        off = mark.fields[2];
    } else if(mark.variant == 'External') {
        url = "/v1/log/external/" + mark.fields[0];
        off = mark.fields[1];
    }
    return {
        type: UPDATE_REQUEST,
        url: url,
        response_type: 'text',
        headers: {'Range': 'bytes=' + off + '-' + (off + 65536)},
        decoder: x => x,
        immediate: true,
    }
}

export function matches_filter(filter, record) {
    if(filter == "")
        return true;
    if(filter == "-")
        return record.role == null;
    if(record.role && record.role.indexOf(filter) >= 0) {
        return true;
    }
    return false;
}

export function filtered(items, filter) {
    let result = []
    for(let item of items) {
        if(matches_filter(filter, item)) {
            result.push(item);
        }
    }
    if(result.length > 200) {
        result.splice(0, result.length-200)
    }
    return result
}
