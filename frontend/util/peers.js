export function servers(peers_json, system_status) {
    return [{
        id: system_status.id,
        hostname: system_status.hostname,
    }].concat(peers_json)
}
