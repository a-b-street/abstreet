use crate::{mut_edits, App, Crossing};

pub fn populate_existing_crossings(app: &mut App) {
    // (Don't call before_edit; this transformation happens before the user starts editing
    // anything)
    for road in app.per_map.map.all_roads() {
        for (dist, kind) in &road.crossing_nodes {
            let list = mut_edits!(app)
                .crossings
                .entry(road.id)
                .or_insert_with(Vec::new);
            list.push(Crossing {
                kind: *kind,
                dist: *dist,
                user_modified: false,
            });
            list.sort_by_key(|c| c.dist);
        }
    }
}
