export function names_filter(filter, name) {
    if(filter == '') {
        return true;
    }
    if(filter.indexOf(',') >= 0) {
        for(var item of filter.split(',')) {
            if(item == name) {
                return true;
            }
        }
        return false;
    }
    return name.indexOf(filter) >= 0
}
