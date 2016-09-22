import {createStore, applyMiddleware} from 'redux'
import {CANCEL} from 'khufu-runtime'


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
    let [path, query] = uri.split('?');
    let chunks = path.split('/')
    if(chunks[0] == '') {
        chunks.shift()
    }
    let q = {}
    if(query) {
        for(let pair of query.split('&')) {
            let [key, value] = pair.split('=')
            q[key] = value || ''
        }
    }
    return {path: chunks, query: q}
}

export function path(state={path:[], query:{}}, action) {
    switch(action.type) {
        case 'update':
            return {path: action.path, query: state.query}
        case 'set_query':
            return {path: action.path, query: {
                [action.key]: action.value,
                ...state.query}}
        case 'silently_clear_query':
        case 'set_query_default':
            let newq = {...state.query}
            delete newq[action.key]
            return {path: action.path, query: newq}
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
    next({type: 'reset', value: deserialize(location.pathname)})
    window.addEventListener('popstate', function(e) {
        next({type: 'reset', value: deserialize(location.pathname)})
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
                    serialize(action.path, {
                        [action.key]: action.value,
                        ...state.query}))
                break;
            case 'silently_clear_query': {
                let newq = {...state.query}
                delete newq[action.key]
                history.replaceState({}, '', serialize(action.path, newq))
                break;
            }
            case 'set_query_default': {
                let newq = {...state.query}
                delete newq[action.key]
                history.pushState({}, '', serialize(action.path, newq))
                break;
            }
        }
        next(action)
    }
}

export var url_query = key => store => next => {
    let defvalue = null;
    return action => {
        switch(action.type) {
            case 'set':
                if(action.value == defvalue) {
                    router.dispatch({
                        type: 'set_query_default',
                        key: key,
                    })
                } else {
                    router.dispatch({
                        type: 'set_query',
                        key: key,
                        value: action.value,
                    })
                }
                break;
            case 'init':
                def_value = action.value;
                break;
            case CANCEL:
                router.dispatch({
                    type: 'silently_clear_query',
                    key: key,
                })
                break;
        }
        return next(action)
    }
}

export var router = createStore(path, applyMiddleware(routing_middleware))
