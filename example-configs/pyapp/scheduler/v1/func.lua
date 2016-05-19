function map(func, array)
  local new_array = {}
  for i,v in ipairs(array) do
    new_array[i] = func(v)
  end
  return new_array
end

function map_pairs(func, array)
  local new_array = {}
  for k,v in pairs(array) do
    new_array[k] = func(k, v)
  end
  return new_array
end

function map_to_dict(func, array)
  local new_array = {}
  for k,v in pairs(array) do
    local pair = func(k, v)
    new_array[pair[1]] = pair[2]
  end
  return new_array
end

function map_reverse(func, array)
  local new_array = {}
  for i=#array,1,-1 do
    new_array[#new_array+1] = func(array[i])
  end
  return new_array
end

function filter(func, array)
  local new_array = {}
  for i,v in ipairs(array) do
    if func(v) then
        new_array[#new_array+1] = v
    end
  end
  return new_array
end

return {
    map=map,
    map_pairs=map_pairs,
    map_reverse=map_reverse,
    map_to_dict=map_to_dict,
    filter=filter,
}
