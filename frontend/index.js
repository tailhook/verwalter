import {createStore, applyMiddleware} from 'redux'
import {attach} from 'khufu-runtime'
import {Router} from 'khufu-routing'

import {main} from './main.khufu'

let prefix = []
if(window.location.pathname.charAt(1) == '~') {
    // check different site root
    prefix = [window.location.pathname.split('/', 2)[1]]
    console.log("Running as frontend named", prefix)
}
let router = new Router(window, prefix);
let khufu_instance = attach(document.getElementById('app'),
    main(router, VERSION), {
    store(reducer, middleware, state) {
        if(typeof reducer != 'function') {
            return reducer
        }
        let mid = middleware.filter(x => typeof x === 'function')
        if(DEBUG) {
            let logger = require('redux-logger')
            mid.push(logger.createLogger({
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

let unsubscribe = router.subscribe(khufu_instance.queue_render)

if(module.hot) {
    module.hot.accept()
    module.hot.dispose(() => {
        unsubscribe()
    })
}
