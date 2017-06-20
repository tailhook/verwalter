export function step_and_index(pipeline, step_name) {
    if(step_name == 'start') {
        return [{name: 'start'}, -1]
    }
    for(let i = 0; i < pipeline.length; ++i) {
        if(pipeline[i].name == step_name) {
            return [pipeline[i], i];
        }
    }
}
