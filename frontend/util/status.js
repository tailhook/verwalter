export function is_leader(status) {
    // TODO(tailhook) Isn't this is ugly check?
    return status.election_state.is_leader;
}
