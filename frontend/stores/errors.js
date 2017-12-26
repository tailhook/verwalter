export function set_error(err) {
    return {type: 'error', error: err}
}

export function clear() {
    return {type: 'clear'}
}

export function error(state=null, action) {
    switch(action.type) {
        case "error":
            return action.error
        case "clear":
            return null
    }
    return state
}
