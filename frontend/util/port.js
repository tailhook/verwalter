export function set_port(host, addr) {
    if(!addr) {
        return `http://${host}:8379`
    }
    let port = addr.split(':')[1];
    return `http://${host}:${port}`
}
