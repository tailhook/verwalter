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

export function set(val) {
    return { type: 'set', value: val }
}
