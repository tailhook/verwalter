import {createStore, applyMiddleware} from 'redux'

function serialize(chunks) {
    return '/' + chunks.join('/')
}

function deserialize(path) {
    let m = path.match(/^https?:\/\/[^\/]+(\/.*)$/)
    if(m) {
        path = m[1]
    }
    let chunks = path.split('/')
    if(chunks[0] == '') {
        chunks.shift()
    }
    return chunks
}

export function path(state=[], action) {
    switch(action.type) {
        case 'update':
            return action.path
        case 'reset':
            return action.value
    }
    return state
}

export function go(delta_or_event, event) {
    let path;
    if(delta_or_event instanceof Event) {
        event = delta_or_event
        path = deserialize(event.currentTarget.href)
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
        if(action.type == 'update') {
            history.pushState({}, '', serialize(action.path))
        }
        next(action)
    }
}

export var router = createStore(path, applyMiddleware(routing_middleware))
