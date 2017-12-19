export function is_leader(status) {
    return status && status.election_state && status.electin_state.is_leader;
}
