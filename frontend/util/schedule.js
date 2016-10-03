export function total_processes(schedule, role_name) {
    let proc = 0;
    let rows = 0;
    for(let host in schedule.nodes) {
        let hrole = schedule.nodes[host].roles[role_name]
        if(hrole) {
            for(let kind in hrole.daemons) {
                proc += hrole.daemons[kind].instances || 0
                rows += 1;
            }
        }
    }
    return [proc, rows]
}
