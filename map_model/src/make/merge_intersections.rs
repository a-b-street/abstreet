use abstutil::MapName;

use crate::raw::{OriginalRoad, RawMap};

/// Experimentally try to merge tiny "roads" that're actually just part of a complicated
/// intersection.
pub fn merge_short_roads(map: &mut RawMap) {
    // TODO A few hardcoded cases to start.
    if map.name == MapName::seattle("phinney") {
        // Aurora & 77th
        for id in vec![
            OriginalRoad::new(158718276, (1424516502, 809059829)),
            OriginalRoad::new(9025494, (1424516502, 670081609)),
        ] {
            if false {
                map.merge_short_road(id).unwrap();
            }
        }
    }
}
