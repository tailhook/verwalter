JSON = require "JSON"
trace = require "trace"
version = require "version"
func = require "func"
merge = require "merge"

function app_num_workers(name, nums, actions, peers)
    print("-------------- ROLE", name, "-----------------")
    if nums == nil then
        nums = {celery=2, worker=1}
    end
    trace.object("state", nums)
    trace.object("actions", actions)
    trace.object("peers", peers)
    func.map(
        function(a)
            nums[a.button.process] = nums[a.button.process] + a.button.incr
        end,
        actions)
    trace.object("node_roles", func.map(function (node) return {
            daemons={
                worker={key="worker", instances=nums.worker,
                        image="v1", config="/cfg/web-worker.yaml"},
                celery={key="celery", instances=nums.celery,
                        image="v1", config="/cfg/celery.yaml"},
            }} end, peers))
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
        node_roles=func.map_pairs(function (node) return {
            daemons={
                worker={key="worker", instances=nums.worker,
                        image="v1", config="/cfg/web-worker.yaml"},
                celery={key="celery", instances=nums.celery,
                        image="v1", config="/cfg/celery.yaml"},
            }} end, peers),
    }
end

function _scheduler(state)
    trace.object("INPUT", state)

    local roles = {app1=app_num_workers}
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

            return role_func(role_name, curstate, actions, peers)
        end, roles)))
end

scheduler = trace.wrap_scheduler(_scheduler)
