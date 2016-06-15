export function enumerate(iter) {
    let i = 0;
    return Array.from(iter).map(val => [i++, val])
}
