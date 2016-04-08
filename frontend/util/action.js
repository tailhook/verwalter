export function execute(data) {
    fetch("/v1/action", {
        method: 'POST',
        body: JSON.stringify(data),
    }).then(response => console.log("Action response", response))
    return {type: 'execute_action', data: data}
}
