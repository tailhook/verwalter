import {DATA} from '../middleware/request'

export default function text(state=null, action) {
    switch(action.type) {
        case DATA:
            return action.data;
    }
    return state;
}
