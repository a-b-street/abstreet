use map_model::Map;

use crate::{Crossing, Edits};

pub fn populate_existing_crossings(map: &Map, edits: &mut Edits) {
    // (Don't call before_edit; this transformation happens before the user starts editing
    // anything)
    for road in map.all_roads() {
        for (dist, kind) in &road.crossing_nodes {
            let list = edits.crossings.entry(road.id).or_insert_with(Vec::new);
            list.push(Crossing {
                kind: *kind,
                dist: *dist,
                user_modified: false,
            });
            list.sort_by_key(|c| c.dist);
        }
    }
}
