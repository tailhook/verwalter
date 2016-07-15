local function _state(state)
    local peer_set = {}
    for _, node in pairs(state.peers) do
        peer_set[node.hostname] = 1
    end
    state.peer_set = peer_set
end

return {
    state=_state,
}
