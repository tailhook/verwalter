import {createStore, applyMiddleware} from 'redux'
import {CANCEL} from 'khufu-runtime'


let QUERIES = {}


function serialize(chunks, query) {
    let q = ''
    for(let k in query) {
        if(!q) {
            q = '?'
        } else {
            q += '&'
        }
        let val = query[k]
        q += encodeURIComponent(k)
        if(val != '') {
             q += '=' + encodeURIComponent(val)
        }
    }
    return '/' + chunks.join('/') + q
}

function deserialize(uri) {
    let m = uri.match(/^https?:\/\/[^\/]+(\/.*)$/)
    if(m) {
        uri = m[1]
    }
    let [page, fragment] = uri.split('#');
    let [path, query] = page.split('?');
    let chunks = path.split('/')
    if(chunks[0] == '') {
        chunks.shift()
    }
    let q = {}
    if(query) {
        for(let pair of query.split('&')) {
            let [key, value] = pair.split('=')
            q[decodeURIComponent(key)] = decodeURIComponent(value || '')
        }
    }
    return {path: chunks, query: q}
}

export function path(state={path:[], query:{}}, action) {
    switch(action.type) {
        case 'update':
            return {path: action.path, query: state.query}
        case 'set_query':
        case 'replace_query':
            return {path: state.path, query: {
                ...state.query,
                [action.key]: action.value}}
        case 'silently_clear_query':
        case 'set_query_default':
            let newq = {...state.query}
            delete newq[action.key]
            return {path: state.path, query: newq}
        case 'reset':
            return action.value
    }
    return state
}

export function go(delta_or_event, event) {
    let path, query;
    if(delta_or_event instanceof Event) {
        event = delta_or_event
        path = deserialize(event.currentTarget.href).path
    } else {
        path = delta_or_event
    }
    if(event) {
        event.preventDefault()
    }
    return {type: 'update', path: path}
}

var routing_middleware = ({getState}) => next => {
    let urlstate = deserialize(location.toString());
    next({type: 'reset', value: urlstate})
    for(var k in urlstate) {
        if(k in QUERIES) {
            QUERIES[k]({type: 'raw_set', value: urlstate.query[k]})
        }
    }
    window.addEventListener('popstate', function(e) {
        let urlstate = deserialize(location.toString());
        let oldq = getState().query;
        next({type: 'reset', value: urlstate})
        for(var k in urlstate.query) {
            if(k in QUERIES) {
                QUERIES[k]({type: 'raw_set', value: urlstate.query[k]})
            }
        }
        for(var k in oldq) {
            if(!(k in urlstate.query) && k in QUERIES) {
                QUERIES[k]({type: 'raw_clear', value: urlstate.query[k]})
            }
        }
    })
    return action => {
        let state = getState()
        switch(action.type) {
            case 'update':
                history.pushState({}, '',
                    serialize(action.path, action.query || state.query))
                break;
            case 'set_query':
                history.pushState({}, '',
                    serialize(state.path, {
                        ...state.query,
                        [action.key]: action.value}))
                break;
            case 'replace_query':
                history.replaceState({}, '',
                    serialize(state.path, {
                        ...state.query,
                        [action.key]: action.value}))
                break;
            case 'silently_clear_query': {
                let newq = {...state.query}
                delete newq[action.key]
                history.replaceState({}, '', serialize(state.path, newq))
                break;
            }
            case 'set_query_default': {
                let newq = {...state.query}
                delete newq[action.key]
                history.pushState({}, '', serialize(state.path, newq))
                break;
            }
        }
        return next(action)
    }
}

export var _query = (key, replace) => ({getState}) => next => {
    let def_value = getState()
    next({type: 'set', value: router.getState().query[key]})
    let handler = action => {
        switch(action.type) {
            case 'raw_set':
                return next({type: 'set', value: action.value})
            case 'raw_clear':
                return next({type: 'set', value: def_value})
            case 'set':
                if(action.value == def_value) {
                    router.dispatch({
                        type: 'set_query_default',
                        key: key,
                    })
                } else {
                    let oldv = router.getState().query[key]
                    if(!replace || oldv == '' || oldv == def_value) {
                        router.dispatch({
                            type: 'set_query',
                            key: key,
                            value: action.value,
                        })
                    } else {
                        router.dispatch({
                            type: 'replace_query',
                            key: key,
                            value: action.value,
                        })
                    }
                }
                break;
            case 'init':
                def_value = action.value;
                break;
            case CANCEL:
                if(QUERIES[key] == handler) {
                    delete QUERIES[key]
                }
                router.dispatch({
                    type: 'silently_clear_query',
                    key: key,
                })
                break;
        }
        return next(action)
    }
    if(QUERIES[key]) {
        console.log("Old store is still hanging for", key)
    }
    QUERIES[key] = handler
    return handler
}

export var url_query = key => _query(key, false)
export var smart_query = key => _query(key, true)

export var router = createStore(path, applyMiddleware(routing_middleware))
