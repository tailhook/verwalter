inspect = require "inspect"

function debugger()
    local x = {text=""}
    function x.object(self, title, data)
        self.text = self.text
            .. string.format('----- %s ----\n', title)
            .. inspect(data)
            .. "\n"
    end
    function x.print(self, ...)
        local text = self.text
        for i, v in pairs({...}) do
            if i > 1 then
                text = text .. " "
            end
            text = text .. tostring(v)
        end
        self.text = text
    end
    return x
end

function wrap_scheduler(real_scheduler)
    return function(state)
        local dbg = debugger()
        _G.print = function(...) dbg:print(...) end
        flag, value = pcall(_scheduler, state, dbg)
        _G.print = nil
        if flag then
            return value, dbg.text
        else
            text = dbg.text .. string.format("\nError: %s", value)
            return nil, text
        end
    end
end

return {
    debugger=debugger,
    wrap_scheduler=wrap_scheduler,
}
