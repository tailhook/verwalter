export function matches_filter(filter, value) {
    if(filter == '') {
        return true;
    }
    if(filter[0] == 'v') {
        return value.indexOf(filter) == 0;
    }
    return value.indexOf(filter) >= 0;
}

export function filter_versions(values, filter) {
    return values.filter(x => matches_filter(filter, x))
}
