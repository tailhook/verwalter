local JSON = require "JSON"
local trace = require "trace"
local func = require "func"
local merge = require "merge"
local split = require "split"
local preprocess = require "preprocess"

local function app_num_workers(props)
    local name = props.role
    local nums = props.parents[0] -- this should be more intelligent
    local actions = props.actions
    local peers = props.peers
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
            template="pyapp/v1",
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
        nodes=func.map_pairs(function (_) return {
            daemons={
                worker={key="worker", instances=nums.worker,
                        image="v1", config="/cfg/web-worker.yaml"},
                celery={key="celery", instances=nums.celery,
                        image="v1", config="/cfg/celery.yaml"},
            }} end, props.peer_set),
    }
end

local function versioned_app(props)
    local state = props.parents[0] -- this should be more intelligent
    local actions = props.actions
    local peers = props.peers
    local all_versions = {'v1.0', 'v1.1', 'v2.0', 'v2.2', 'v3.1'}
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
    local nodes = func.map_pairs(function (_) return {
            template="pyapp/v1",
            daemons={
                worker={key="worker", instances=1,
                        image="worker."..state.version,
                        config="/cfg/web-worker.yaml"},
                celery={key="celery", instances=2,
                        image="celery."..state.version,
                        config="/cfg/celery.yaml"},
            }} end, props.peer_set)
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

local function rich_migration_app(props)
    local state = props.parents[0] -- this should be more intelligent
    local actions = props.actions
    local peers = props.peers
    local all_versions = {'v1.0', 'v1.1', 'v2.0', 'v2.2', 'v3.1'}
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
    local nodes = func.map_pairs(function (_) return {
            template="pyapp/v1",
            daemons={
                worker={key="worker", instances=1,
                        image="worker."..state.version,
                        config="/cfg/web-worker.yaml"},
                celery={key="celery", instances=2,
                        image="celery."..state.version,
                        config="/cfg/celery.yaml"},
            }} end, props.peer_set)
    if not state.running then
        nodes = {}
    end
    return {
        state=state,
        info=status_processes,
        role={
            frontend={
                kind='migration', allow_stop=true,
                migration={
                    -- We need some interesting timestamps
                    -- but can't always return `now`, because it means
                    -- schedule is changed too often
                    -- So we just round them up
                    start_time=math.floor((props.now - 30*60*1000)
                                        /3600000)*3600000,
                    timestamp=math.floor(props.now/600000)*600000,
                    original='v1.0',
                    target='v2.2',
                    progress=0.75,
                    mode="backward",
                    status_title="Growing",
                    buttons={
                        {id='test', title='test',
                         description='Start test'},
                        {id='pause', icon='pause',
                         description='Pause migration'},
                        {id='revert', kind='danger', icon='backward',
                         description='Revert'},
                        {id='apply', icon='forward',
                         description='Switch now'},
                    },
                },
                bottom_text=[[This interface doesn't work because we
                              have only implemented a frontend but not
                              a respective scheduler example.
                              With proper scripting it works like charm!]],
                version_buttons={
                    {id='start', kind='success', title='Start Migration'},
                    {id='quick_switch', kind='danger', title='Force'},
                }
            },
            versions=all_versions,
        },
        nodes=nodes,
    }
end

local function _scheduler(state)
    local roles = {
        app1=app_num_workers,
        app2=versioned_app,
        app3=rich_migration_app,
    }
    preprocess.state(state)
    return JSON:encode(merge.schedules(
        func.map_pairs(function (role_name, role_func)
            print("-------------- ROLE", role_name, "-----------------")
            return role_func {
                role=role_name,
                runtime=state.runtime[role_name],
                actions=split.actions(state, role_name),
                parents=split.states(state, role_name),
                metrics=split.metrics(state, role_name),
                peers=state.peers,
                peer_set=state.peer_set,
                now=state.now,
            }
        end, roles)))
end

return {
    scheduler=trace.wrap_scheduler(_scheduler),
}
