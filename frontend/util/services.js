export function role_nodes(schedule, role) {
    let res = {}
    for(let node_name in schedule.data.nodes) {
        let node = schedule.data.nodes[node_name];
        let val = node.roles && node.roles[role]
        if(val) {
            res[node_name] = val
        }
    }
    return res
}
