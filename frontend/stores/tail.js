export const FOLLOW = '@@tail/follow'
export const LOAD_PREVIOUS = '@@tail/load_previous'
export const ERROR = '@@tail/error'
export const CHUNK = '@@tail/chunk'
const CHUNK_SIZE = 512

function log_state() {
    return {
        byte_offset: null,
        str_offset: null,
        loading: true,
        total: 0,
        bytes: new Uint8Array(),
        string: null,
        follow: true,
    }
}

export function tail(state=log_state(), action) {
    switch(action.type) {
        case CHUNK:
            // TODO(tailhook) concat
            let nbytes = new Uint8Array(action.bytes)
            let noffset = action.offset
            let nsoffset = action.offset
            let line_off = 0
            if(nsoffset != 0) {
                line_off = nbytes.findIndex(c => c == 10)
                if(line_off !== undefined) {
                    line_off += 1
                    nsoffset += line_off
                } else {
                    line_off = 0
                }
            }
            let nstr = new TextDecoder().decode(nbytes.slice(line_off));
            return {...state,
                loading: false,
                byte_offset: noffset,
                str_offset: nsoffset,
                str_end: noffset + nbytes.length - line_off,
                total: action.total,
                bytes: nbytes,
                string: nstr,
            }
        case FOLLOW:
            return {...state, follow: action.enable}
        default:
            return state;
    }
}

export var tailer = uri => store => next => {
    var request
    var timeout
    var follow
    var load_before

    function start() {
        if(timeout) {
            clearTimeout(timeout)
            timeout = null
        }

        request = new XMLHttpRequest();
        var time = new Date();
        request.responseType = "arraybuffer"
        request.onreadystatechange = (ev) => {
            if(request.readyState < 4) {
                return;
            }
            var lcy = new Date() - time;
            let req  = request;

            request = null; // not processing any more
            if(store.getState().follow) {
                timeout = setTimeout(start, 500)
            }

            if(req.status != 206) {
                next({type: ERROR, request: req, latency: lcy})
                return;
            }
            let [range, total] = req.getResponseHeader('Content-Range').split('/');
            let [chunk_start, end] = range.split('-');
            try {
                next({
                    type: CHUNK,
                    bytes: req.response,
                    offset: parseInt(chunk_start),
                    len: parseInt(end) - parseInt(chunk_start) + 1,
                    total: parseInt(total),
                    latency: lcy,
                })
            } catch(e) {
                next({type: ERROR, exception: e, latency: lcy})
            }
        }
        request.open('GET', uri, true);
        let cur_off = store.getState().byte_offset;
        let end_off = cur_off + store.getState().bytes.length;
        if(load_before != null && cur_off > load_before) {
            request.setRequestHeader('Range',
                'bytes='+load_before+'-'+(
                    cur_off == null ? load_before + CHUNK_SIZE : cur_off-1 ));
            load_before = null;
        } else if(cur_off != null) {
            console.log(end_off)
            request.setRequestHeader('Range',
                'bytes='+(end_off > 0 ? end_off-1 : 0)+'-'+ (end_off + CHUNK_SIZE));
        } else {
            request.setRequestHeader('Range', 'bytes=-'+CHUNK_SIZE);
        }
        request.send()
    }
    function stop() {
        if(request) {
            request.onreadystatechange = null
            request.abort()
            request = null
        }
        if(timeout) {
            clearTimeout(timeout)
            timeout = null
        }
    }
    start()
    return action => {
        switch(action.type) {
            case FOLLOW:
                follow = action.enable
                if(follow && !request) {
                    start()
                }
                break;
            case LOAD_PREVIOUS:
                load_before = Math.max(
                    store.getState().byte_offset - CHUNK_SIZE, 0);
                if(!request) {
                    start()
                }
                break;
            case CANCEL:
                stop();
                break;
        }
        return next(action)
    }
}

export function follow(val) {
    return {type: FOLLOW, enable: val}
}

export function load_previous() {
    return {type: LOAD_PREVIOUS}
}
