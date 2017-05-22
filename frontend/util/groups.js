export function count_instances(group) {
    let counter = 0
    for(let k in group.services) {
        let svc = group.services[k];
        counter += svc.number_per_server * svc.servers.length
    }
    return counter
}
