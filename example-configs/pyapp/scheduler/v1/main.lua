JSON = require "JSON"
debug = require "debug"
version = require "version"
func = require "func"

function cycle(items)
    local i = 0
    local n = #items
    return function()
        i = i + 1
        if i > n then i = 1 end
        return items[i]
    end
end

function _scheduler(state, debugger)
    debugger:object("INPUT", state)
    local template_version = "v1"

    local available_versions = {}
    for ver, _ in pairs(state.roles.pyapp.runtime) do
        available_versions[#available_versions+1] = ver
    end
    table.sort(available_versions, version.compare)

    -- First, if someone pressed the button, just use lastest version pressed
    -- Button name is actually a version in our case
    local versions = func.map(
        function(action)
            return action.button
        end,
        state.actions)

    -- Naive algorithm: get the biggest version in every parent schedule
    -- TODO(tailhook) better idea it to get the one with the latest timestamp
    if #versions == 0 then
        versions = func.map(
            function(s) return s.role_metadata.pyapp.info.version end,
            state.parents)
        -- If there was no previous schedules, use the latest existing config
        if #versions == 0 then
            versions = available_versions
        end
        -- Sort and get the latest/biggest one
        table.sort(versions, version.compare)
    end
    local runtime_version = versions[#versions]

    local runtime = state.roles.pyapp.runtime[runtime_version]
    local req = runtime.required_processes
    local node_list = {"n1", "n2", "n3"}

    -- In this example we assume that all processes are equally costly
    -- TODO(tailhook) account already running things, so we do as little
    --                process migrations as possible
    local nodes = cycle(node_list)
    local counts = {}
    for name, number in pairs(req) do
        for i = 0,number,1 do
            local node_name = nodes()
            node = counts[node_name]
            if node == nil then
                node = {}
                counts[node_name] = node
            end
            oldval = node[name]
            if oldval == nil then oldval = 0 end
            node[name] = oldval + 1
        end
    end

    local nodes = {}
    for name, processes in pairs(counts) do
        items = {}
        for proc, num in pairs(processes) do
            proccfg = runtime.processes.daemons[proc]
            items[#items + 1] = {
                key=proc,
                image=proccfg.image,
                config=proccfg.config,
                instances=num,
            }
        end
        nodes[name] = {
            pyapp={
                daemons=items,
            },
        }
    end
    result = {
        role_metadata={
            pyapp={
                commands={},
                templates="v1",
                badge=runtime_version,
                info={
                    version=runtime_version,
                },
                buttons=func.map_reverse(function (v)
                    return {id=v,
                            title="Switch to " .. v}
                    end, available_versions),
            },
        },
        nodes=nodes,
    }
    return JSON:encode(result)
end

scheduler = debug.wrap_scheduler(_scheduler)
