use abstutil::MapName;

use crate::raw::{OriginalRoad, RawMap};

/// Experimentally try to merge tiny "roads" that're actually just part of a complicated
/// intersection.
pub fn merge_short_roads(map: &mut RawMap) {
    // TODO A few hardcoded cases to start.
    if map.name == MapName::seattle("phinney") {
        for id in vec![
            // Aurora & 77th
            OriginalRoad::new(158718276, (1424516502, 809059829)),
            OriginalRoad::new(9025494, (1424516502, 670081609)),
            // Aurora & Winoa
            OriginalRoad::new(331588704, (30759900, 670081611)),
        ] {
            if false {
                map.merge_short_road(id).unwrap();
            }
        }
    }
}
