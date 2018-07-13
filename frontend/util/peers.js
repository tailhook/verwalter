export function servers(peers_json, system_status) {
    let servers = [{
        id: system_status.id,
        hostname: system_status.hostname,
    }].concat(peers_json)
    servers.sort((a, b) => a.hostname.localeCompare(b.hostname))
    return servers
}
