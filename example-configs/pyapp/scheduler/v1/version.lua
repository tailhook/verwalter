local string = require "string"

local function compare(a, b)
    if a:sub(1, 1) == 'v' then a = a:sub(2) end
    if b:sub(1, 1) == 'v' then b = b:sub(2) end
    local aiter = string.gmatch(a, "%w+")
    local biter = string.gmatch(b, "%w+")
    while true do
        local aitem = aiter()
        local bitem = biter()
        if aitem == nil then return bitem ~= nil end
        if bitem == nil then return false end
        if aitem ~= bitem then
            if string.match(aitem, "^%d+$") then
                if string.match(bitem, "^%d+$") then
                    local anum = tonumber(aitem)
                    local bnum = tonumber(bitem)
                    return anum < bnum
                else -- numbers are always less than letters
                    return false
                end
            else
                if string.match("%d+", bitem) then
                    -- numbers are always less than letters
                    return true
                else
                    return aitem < bitem
                end
            end
        end
    end
end

return {
    compare=compare,
}
