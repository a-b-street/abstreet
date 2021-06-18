use anyhow::Result;

use map_model::raw::{RawBusRoute, RawBusStop, RawMap};

pub fn import_gtfs(
    map: &mut RawMap,
    path: &str,
) -> Result<()> {
    // Fill out the RawBusRoutes
    Ok(())
}
