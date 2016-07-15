local func = require('func')

local function get_actions(state, role_name)
    return func.filter(
        function (a) return a.button.role == role_name end,
        state.actions)
end

local function get_states(state, role_name)
    local curstates = {}
    for _, par in pairs(state.parents) do
        if par.state ~= nil and par.state[role_name] ~= nil then
            curstates[#curstates+1] = par.state[role_name]
        end
    end
    return curstates
end

local function get_metrics(state, role_name)
    local pattern = "^" .. role_name:gsub("-", "%%-") .. "%.(.+)$"
    local metrics = {}
    local trace = require('trace')

    if state.metrics then
        metrics = func.filter_pairs(
            function (node_id, node_metrics)
                local peer = state.peers[node_id]
                if peer == nil then
                    return nil, nil
                end

                return peer.hostname, func.filter_pairs(
                    function (k, v)
                        local localk = k:match(pattern)
                        if localk ~= nil then
                            return localk, v
                        end
                    end,
                    node_metrics)
            end,
            state.metrics)
    end
    return metrics
end

return {
    actions=get_actions,
    states=get_states,
    metrics=get_metrics,
}
