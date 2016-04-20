inspect = require "inspect"

local text = nil

function object(title, data)
    text = text
        .. string.format('----- %s ----\n', title)
        .. inspect(data)
        .. "\n"
end

function print(...)
    for i, v in pairs({...}) do
        if i > 1 then
            text = text .. " "
        end
        text = text .. tostring(v)
    end
    text = text .. "\n"
end

_G.print = print

function wrap_scheduler(real_scheduler)
    return function(state)
        text = ""
        local flag, value = pcall(_scheduler, state)
        local current_text = text
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
