export function step_and_index(pipeline, step_name) {
    if(step_name == 'start' || step_name == 'revert_done') {
        return [{name: 'start'}, -1]
    }
    if(step_name == 'done') {
        return [{name: 'done'}, pipeline.length]
    }
    for(let i = 0; i < pipeline.length; ++i) {
        if(pipeline[i].name == step_name) {
            return [pipeline[i], i];
        }
    }
    console.error("Invalid step", step_name, "in pipeline", pipeline)
}
