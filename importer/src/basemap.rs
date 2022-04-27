use anyhow::Result;

use abstutil::Timer;
use map_model::{EditCmd, Map, MapEdits};

pub fn override_sidewalk_widths(map: &mut Map, path: String, timer: &mut Timer) -> Result<()> {
    let edits = MapEdits::load_from_file(map, path, timer)?;

    for cmd in &edits.commands {
        if let EditCmd::ChangeRoad { r, new, .. } = cmd {
            // When the UI resets a road to its default state, it parses lane specs from OSM tags.
            // Insert OSM tags and pretend that we have upstream data.
            let left = &new.lanes_ltr[0];
            if left.lt.is_walkable() {
                map.mut_road(*r)
                    .osm_tags
                    .insert("sidewalk:left:width", left.width.inner_meters().to_string());
            }

            let right = new.lanes_ltr.last().unwrap();
            if right.lt.is_walkable() {
                map.mut_road(*r).osm_tags.insert(
                    "sidewalk:right:width",
                    right.width.inner_meters().to_string(),
                );
            }
        }
    }

    // Also just apply the edits, so that lanes stored in the map are updated
    map.must_apply_edits(edits, timer);
    map.recalculate_pathfinding_after_edits(timer);
    map.clear_edits_before_save();

    Ok(())
}
