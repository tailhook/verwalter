export function preloader(state={}, action) {
    return state
}

export function action(name) {
    console.log("EXECUTING", name)
    fetch("/v1/" + name, {
        method: 'POST',
    }).then(response => console.log("Global action response", response))
    return {type: 'execute_action', action: name}
}
