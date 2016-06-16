local function map(func, array)
  local new_array = {}
  for i,v in ipairs(array) do
    new_array[i] = func(v)
  end
  return new_array
end

local function map_pairs(func, array)
  local new_array = {}
  for k,v in pairs(array) do
    new_array[k] = func(k, v)
  end
  return new_array
end

local function map_to_dict(func, array)
  local new_array = {}
  for k,v in pairs(array) do
    local nk, nv = func(k, v)
    new_array[nk] = nv
  end
  return new_array
end

local function map_to_array(func, array)
  local new_array = {}
  for k,v in pairs(array) do
    local nv = func(k, v)
    new_array[#new_array+1] = nv
  end
  return new_array
end

local function map_reverse(func, array)
  local new_array = {}
  for i=#array,1,-1 do
    new_array[#new_array+1] = func(array[i])
  end
  return new_array
end

local function filter(func, array)
  local new_array = {}
  for _, v in ipairs(array) do
    if func(v) then
        new_array[#new_array+1] = v
    end
  end
  return new_array
end

local function filter_pairs(func, dict)
  local items = {}
  for k, v in pairs(dict) do
    local nk, nv = func(k, v)
    if nk ~= nil then
        items[nk] = nv
    end
  end
  return items
end

return {
    map=map,
    map_pairs=map_pairs,
    map_reverse=map_reverse,
    map_to_dict=map_to_dict,
    map_to_array=map_to_array,
    filter=filter,
    filter_pairs=filter_pairs,
}
