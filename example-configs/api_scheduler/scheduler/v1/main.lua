local log = require("toolbox/modules/log")
local roles_from_state = require("toolbox/modules/role").from_state
local merge_output = require("toolbox/modules/role").merge_output
local api = require('toolbox/modules/drivers/api.lua')

local function scheduler(state)
    local roles = roles_from_state { state,
        driver=function(_) return api end,
    }
    return merge_output(roles)
end

return {
    scheduler=log.wrap_scheduler(scheduler),
}
