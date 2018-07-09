export function execute(data) {
    fetch("/v1/action", {
        method: 'POST',
        headers: {
            'Content-Type': 'application/json',
        },
        body: JSON.stringify(data),
    }).then(response => console.log("Action response", response))
    return {type: 'execute_action', data: data}
}

export function compare_action(act, rule) {
    let btn = act.button
    if(!btn) {
        return false;
    }
    for(var k in rule) {
        if(btn[k] != rule[k]) {
            return false;
        }
    }
    return true;
}
