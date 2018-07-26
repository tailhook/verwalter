export function servers(peers_json, system_status) {
    let servers = [];
    if(peers_json) {
        servers = servers.concat(peers_json)
    }
    if(system_status) {
        servers.push({
            id: system_status.id,
            hostname: system_status.hostname,
        });
    }
    servers.sort((a, b) => a.hostname.localeCompare(b.hostname))
    return servers
}
