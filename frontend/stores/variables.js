import {CANCEL} from 'khufu-runtime'


export function variables(state={types: {}, values: {}}, action) {
    switch(action.type) {
        case 'set_var':
            let typ = state.types[action.key]
            let value = validate_type(typ, action.value)
            let values = {...state.values}
            values[action.key] = value
            return {
                values: values,
                types: state.types,
            }
            return nresult
        case 'set_types':
            let nvalues = {}
            for(let key in action.types) {
                if(key in state) {
                    nvalues[key] = validate_type(action.types[key], state[key])
                }
            }
            return {types: action.types, values: nvalues}
        default:
            return state
    }
}

export function set(key, value) {
    return {type: 'set_var', key: key, value: value}
}

export function set_types(types) {
    return {type: 'set_types', types: types}
}

function validate_type(typ, value) {
    switch(typ.type) {
        case "TcpPort":
            return parseInt(value)
    }
}
