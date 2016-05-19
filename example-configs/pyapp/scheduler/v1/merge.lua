function merge_schedules(list)
    local result = {
        state={},
        roles={},
        nodes={},
    }
    for role_name, info in pairs(list) do
        result.state[role_name] = info.state
        result.roles[role_name] = info.role
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
    return result
end

function merge_tables(all_tables)
    local result = {}
    for _, dic in pairs(all_tables) do
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
