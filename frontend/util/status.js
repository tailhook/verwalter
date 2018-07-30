export function is_leader(status) {
    return status && status.election && status.election.isLeader;
}
