import {DATA} from '../middleware/request'

export default function pending_actions(state={}, action) {
    switch(action.type) {
        case DATA:
            return action.data;
        case "execute_action":
            return {[+new Date]: action.data, ...state}

    }
    return state;
}
