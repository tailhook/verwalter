JSON = require "JSON"
trace = require "trace"
version = require "version"
func = require "func"
merge = require "merge"

function app_num_workers(name, nums, actions, peers)
    if nums == nil then
        nums = {celery=2, worker=1}
    end
    func.map(
        function(a)
            nums[a.button.process] = nums[a.button.process] + a.button.incr
        end,
        actions)
    return {
        state=nums,
        role={
            frontend={kind='example'},
            buttons={
                {title="Incr celery",
                 action={process='celery', incr=1, role=name}},
                {title="Decr celery",
                 action={process='celery', incr=-1, role=name}},
                {title="Incr workers",
                 action={process='worker', incr=1, role=name}},
                {title="Decr workers",
                 action={process='worker', incr=-1, role=name}},
            },
        },
        nodes=func.map_pairs(function (node) return {
            daemons={
                worker={key="worker", instances=nums.worker,
                        image="v1", config="/cfg/web-worker.yaml"},
                celery={key="celery", instances=nums.celery,
                        image="v1", config="/cfg/celery.yaml"},
            }} end, peers),
    }
end

function versioned_app(name, state, actions, peers)
    all_versions = {'v1.0', 'v1.1', 'v2.0', 'v2.2', 'v3.1'}
    if state == nil then
        state = {version='v2.0', running=true}
    end
    func.map( -- use a version from latests pressed button
        function(a)
            if a.button.version then
                state.version = a.button.version
            elseif a.button.stop then
                state.running = false
            elseif a.button.start then
                state.running = true
            end
        end,
        actions)
    local nodes = func.map_pairs(function (node) return {
            daemons={
                worker={key="worker", instances=1,
                        image="worker."..state.version,
                        config="/cfg/web-worker.yaml"},
                celery={key="celery", instances=2,
                        image="celery."..state.version,
                        config="/cfg/celery.yaml"},
            }} end, peers)
    if not state.running then
        nodes = {}
    end
    return {
        state=state,
        role={
            frontend={kind='version', allow_stop=true},
            versions=all_versions,
        },
        nodes=nodes,
    }
end

function _scheduler(state)
    trace.object("INPUT", state)

    local roles = {
        app1=app_num_workers,
        app2=versioned_app,
    }
    local peers = func.map_to_dict(
        function (_, node) return {node.hostname, node} end,
        state.peers)

    return JSON:encode(merge.schedules(
        func.map_pairs(function (role_name, role_func)

            local curstate = nil
            for i, par in pairs(state.parents) do
                if par.state ~= nil and par.state[role_name] ~= nil then
                    curstate = par.state[role_name]
                    break
                end
            end

            local actions = func.filter(
                function (a) return a.button.role == role_name end,
                state.actions)

            print("-------------- ROLE", role_name, "-----------------")
            return role_func(role_name, curstate, actions, peers)
        end, roles)))
end

scheduler = trace.wrap_scheduler(_scheduler)
