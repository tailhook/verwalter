export function total_processes(schedule, role_name) {
    let num = 0;
    for(let host in schedule.nodes) {
        let hrole = schedule.nodes[host].roles[role_name]
        for(let kind in hrole.daemons) {
            num += hrole.daemons[kind].instances || 0
        }
    }
    return num
}
