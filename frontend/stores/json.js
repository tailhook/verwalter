import {DATA} from '../middleware/request'

export default function json(state=null, action) {
    switch(action.type) {
        case DATA:
            return action.data;
    }
    return state;
}
