function map(func, array)
  local new_array = {}
  for i,v in ipairs(array) do
    new_array[i] = func(v)
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

return {
    map=map,
    map_reverse=map_reverse,
}
