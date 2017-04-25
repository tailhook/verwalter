local log = require("toolbox/modules/log")
-- local merge = require("toolbox/modules/merge")
local roles_from_state = require("toolbox/modules/role").from_state
local merge_output = require("toolbox/modules/role").merge_output

local function scheduler(state)
    local roles = roles_from_state(state)
    for _, role in pairs(roles) do
        role.independent_scheduling()
    end
    return merge_output(roles)
end

return {
    scheduler=log.wrap_scheduler(scheduler),
}
