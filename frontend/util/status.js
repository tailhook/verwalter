export function is_leader(status) {
    return status && status.election_state && status.election_state.is_leader;
}
