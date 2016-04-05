import {DATA} from '../middleware/request'

export default function json(state={}, action) {
    switch(action.type) {
        case DATA:
            return action.data;
    }
    return state;
}
