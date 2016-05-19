JSON = require "JSON"
trace = require "trace"
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

function _scheduler(state)
    trace.object("INPUT", state)

    nums = state.parents and #state.parents > 0 and
           state.parents[1].processes or {celery=2, worker=1}
    for _, act in pairs(state.actions) do
        nums[act.button.process] = nums[act.button.process] + act.button.incr
    end

    nodes = {}
    for _, node in pairs(state.peers) do
        nodes[node.hostname] = {
            roles={
                pyapp={
                    daemons={
                        worker={key="worker", instances=nums.worker,
                                image="v1", config="/cfg/web-worker.yaml"},
                        celery={key="celery", instances=nums.celery,
                                image="v1", config="/cfg/celery.yaml"},
                    },
                },
            },
        }
    end
    result = {
        roles={
            pyapp={
                frontend={kind='example'},
                buttons={
                    {title="Incr celery", action={process='celery', incr=1}},
                    {title="Decr celery", action={process='celery', incr=-1}},
                    {title="Incr workers", action={process='worker', incr=1}},
                    {title="Decr workers", action={process='worker', incr=-1}},
                },
            }
        },
        nodes=nodes,
        processes=nums,
        query_metrics={
          ["rules"] = {
            ["q1"] = {
              ["series"] = {
                source="Fine",
                condition={"RegexLike", "metric", "^memory\\."},
              },
              extract={"Tip"},
              functions={},
            },
            ["q2"]={
              series={
                source="Fine",
                condition={"RegexLike", "metric", "^cpu\\."}
              },
              extract={"HistoryByNum", 150},
              functions={{"NonNegativeDerivative"},
                         {"SumBy", "metric", "Ignore", true}},
            },
          }
        }
    }
    return JSON:encode(result)
end

scheduler = trace.wrap_scheduler(_scheduler)
