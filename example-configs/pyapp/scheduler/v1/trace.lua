local inspect = require "inspect"

local text = nil

local function object(title, data)
    text = text
        .. string.format('----- %s ----\n', title)
        .. inspect(data)
        .. "\n"
end

local function print(...)
    for i, v in pairs({...}) do
        if i > 1 then
            text = text .. " "
        end
        text = text .. tostring(v)
    end
    text = text .. "\n"
end


local function wrap_scheduler(real_scheduler)
    return function(state)
        local original_print = _G.print
        text = ""
        _G.print = print

        local flag, value = xpcall(
            function() return real_scheduler(state) end,
            debug.traceback)

        local current_text = text
        _G.print = original_print
        text = nil

        if flag then
            return value, current_text
        else
            return nil, current_text .. string.format("\nError: %s", value)
        end
    end
end

return {
    object=object,
    print=print,
    wrap_scheduler=wrap_scheduler,
}
