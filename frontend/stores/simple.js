export function bool(state=undefined, action) {
    switch(action.type) {
        case 'init':
            if(state === undefined) {
                return action.value;
            } else {
                return state;
            }
        case 'enable': return true;
        case 'disable': return false;
        default: return state;
    }
}

export function init(val) {
    return { type: 'init', value: val }
}

export function enable() {
    return { type: 'enable' }
}
export function disable() {
    return { type: 'disable' }
}
export function toggle(value) {
    if(value) {
        return disable()
    } else {
        return enable()
    }
}

export function add(value) {
    return { type: 'add', value: value }
}
export function remove(value) {
    return { type: 'remove', value: value }
}
export function toggleunique(value) {
    return { type: 'toggle', value: value }
}
export function value(state=undefined, action) {
    switch(action.type) {
        case 'init':
            if(state === undefined) {
                return action.value;
            } else {
                return state;
            }
        case 'set':
            return action.value;
        default: return state;
    }
}

export function uniqueset(state=undefined, action) {
    switch(action.type) {
        case 'init':
            if(state === undefined) {
                return action.value
            } else {
                return state
            }
        case 'add':
            return {[action.value]:true, ...state}
        case 'remove': {
            let newstate = {...state}
            delete newstate[action.value]
            return newstate
        }
        case 'toggle': {
            let newstate = {...state}
            if(newstate[action.value]) {
                delete newstate[action.value]
            } else {
                newstate[action.value] = true
            }
            return newstate
        }
        default: return state;
    }
}

export function set(val) {
    return { type: 'set', value: val }
}
