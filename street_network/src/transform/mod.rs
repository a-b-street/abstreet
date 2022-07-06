use abstutil::Timer;

use crate::StreetNetwork;

mod collapse_intersections;
mod dual_carriageways;
mod find_short_roads;
mod merge_short_road;
mod remove_disconnected;
mod shrink_roads;
mod snappy;

impl StreetNetwork {
    // TODO I suspect we'll soon take a full struct of options, maybe even a list of transformation
    // enums to run in order
    /// Run a sequence of transformations to the StreetNetwork before converting it to a full Map.
    ///
    /// We don't want to run these during the OSM->StreetNetwork import stage, because we want to use the
    /// map_editor tool to debug the StreetNetwork.
    pub fn run_all_simplifications(
        &mut self,
        consolidate_all_intersections: bool,
        remove_disconnected: bool,
        timer: &mut Timer,
    ) {
        timer.start("simplify StreetNetwork");

        timer.start("trimming dead-end cycleways (round 1)");
        collapse_intersections::trim_deadends(self);
        timer.stop("trimming dead-end cycleways (round 1)");

        timer.start("snap separate cycleways");
        snappy::snap_cycleways(self);
        timer.stop("snap separate cycleways");

        // More dead-ends can be created after snapping cycleways. But also, snapping can be easier
        // to do after trimming some dead-ends. So... just run it twice.
        timer.start("trimming dead-end cycleways (round 2)");
        collapse_intersections::trim_deadends(self);
        timer.stop("trimming dead-end cycleways (round 2)");

        if remove_disconnected {
            remove_disconnected::remove_disconnected_roads(self, timer);
        }

        timer.start("merging short roads");
        find_short_roads::find_short_roads(self, consolidate_all_intersections);
        merge_short_road::merge_all_junctions(self);
        timer.stop("merging short roads");

        timer.start("collapsing degenerate intersections");
        collapse_intersections::collapse(self);
        timer.stop("collapsing degenerate intersections");

        timer.start("shrinking overlapping roads");
        shrink_roads::shrink(self, timer);
        timer.stop("shrinking overlapping roads");

        timer.stop("simplify StreetNetwork");
    }
}
