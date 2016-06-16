local function merge_schedules(list)
    local result = {
        state={},
        roles={},
        nodes={},
        query_metrics={rules={}},
    }
    for role_name, info in pairs(list) do
        if info ~= nil then
            result.state[role_name] = info.state
            result.roles[role_name] = info.role
            if info.metrics ~= nil then
                for key, value in pairs(info.metrics) do
                    result.query_metrics.rules[role_name .. '.' .. key] = value
                end
            end
            for node_name, node_role in pairs(info.nodes) do
                local mnode = result.nodes[node_name]
                if mnode == nil then
                    mnode = {
                        roles={},
                    }
                    result.nodes[node_name] = mnode
                end
                mnode.roles[role_name] = node_role
            end
        end
    end
    return result
end

local function merge_tables(...)
    local result = {}
    for _, dic in ipairs({...}) do
        for k, v in pairs(dic) do
            result[k] = v
        end
    end
    return result
end

return {
    schedules=merge_schedules,
    tables=merge_tables,
}
