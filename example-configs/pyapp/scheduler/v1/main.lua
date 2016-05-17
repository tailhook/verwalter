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

    nodes = {}
    for _, node in pairs(state.peers) do
        nodes[node.hostname] = {
            vars={
                daemons={
                    worker={key="worker", instances=1,
                            image="v1", config="/cfg/web-worker.yaml"},
                    celery={key="celery", instances=2,
                            image="v1", config="/cfg/celery.yaml"},
                },
            }
        }
    end
    result = {
        roles={
            pyapp={
                template='pyapp/v1',
                commands={},
            }
        },
        nodes=nodes,
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
