export function is_leader(status) {
    // TODO(tailhook) Isn't this is ugly check?
    return status.scheduler_state.substr(0, 7) == 'leader:'
}
