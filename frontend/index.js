import {createStore, applyMiddleware} from 'redux'
import khufu from 'khufu-runtime'

import {main} from './main.khufu'


let khufu_instance = khufu(document.getElementById('app'), main(VERSION), {
    store(reducer, middleware, state) {
        let mid = middleware.filter(x => typeof x === 'function')
        if(DEBUG) {
            let createLogger = require('redux-logger')
            mid.push(createLogger({
                collapsed: true,
            }))
        }
        let store = createStore(reducer, state, applyMiddleware(...mid))
        for(var m of middleware) {
            if(typeof m !== 'function') {
                if(m.type) {
                    store.dispatch(m)
                } else if(DEBUG) {
                    console.error("Wrong middleware", m)
                    throw Error("Wrong middleware: " + m)
                }
            }
        }
        return store
    }
})

if(module.hot) {
    module.hot.accept()
}