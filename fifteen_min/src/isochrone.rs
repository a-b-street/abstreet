use std::collections::HashMap;

use abstutil::MultiMap;
use geom::{Duration, Polygon};
use map_gui::tools::{amenity_type, Grid};
use map_gui::SimpleApp;
use map_model::{connectivity, BuildingID, Map, PathConstraints, PathRequest};
use widgetry::{Color, Drawable, EventCtx, GeomBatch};

/// Represents the area reachable from a single building.
pub struct Isochrone {
    /// The center of the isochrone
    pub start: BuildingID,
    /// What mode of travel we're using
    pub constraints: PathConstraints,
    /// Colored polygon contours, uploaded to the GPU and ready for drawing
    pub draw: Drawable,
    /// How far away is each building from the start?
    pub time_to_reach_building: HashMap<BuildingID, Duration>,
    /// Per category of amenity (defined by helpers::amenity_type), what buildings have that?
    pub amenities_reachable: MultiMap<&'static str, BuildingID>,
}

impl Isochrone {
    pub fn new(
        ctx: &mut EventCtx,
        app: &SimpleApp,
        start: BuildingID,
        constraints: PathConstraints,
    ) -> Isochrone {
        let time_to_reach_building =
            connectivity::all_costs_from(&app.map, start, Duration::minutes(15), constraints);
        let draw = draw_isochrone(app, &time_to_reach_building).upload(ctx);

        let mut amenities_reachable = MultiMap::new();
        for b in time_to_reach_building.keys() {
            let bldg = app.map.get_b(*b);
            for amenity in &bldg.amenities {
                if let Some(category) = amenity_type(&amenity.amenity_type) {
                    amenities_reachable.insert(category, bldg.id);
                }
            }
        }

        Isochrone {
            start,
            constraints,
            draw,
            time_to_reach_building,
            amenities_reachable,
        }
    }

    pub fn path_to(&self, map: &Map, destination_id: BuildingID) -> Option<map_model::Path> {
        let path_request = PathRequest {
            start: map.get_b(self.start).sidewalk_pos,
            end: map.get_b(destination_id).sidewalk_pos,
            constraints: self.constraints,
        };

        map.pathfind(path_request)
    }
}

fn draw_isochrone(
    app: &SimpleApp,
    time_to_reach_building: &HashMap<BuildingID, Duration>,
) -> GeomBatch {
    // To generate the polygons covering areas between 0-5 mins, 5-10 mins, etc, we have to feed
    // in a 2D grid of costs. Use a 100x100 meter resolution.
    let bounds = app.map.get_bounds();
    let resolution_m = 100.0;
    // The costs we're storing are currenly durations, but the contour crate needs f64, so
    // just store the number of seconds.
    let mut grid: Grid<f64> = Grid::new(
        (bounds.width() / resolution_m).ceil() as usize,
        (bounds.height() / resolution_m).ceil() as usize,
        0.0,
    );

    // Calculate the cost from the start building to every other building in the map
    for (b, cost) in time_to_reach_building {
        // What grid cell does the building belong to?
        let pt = app.map.get_b(*b).polygon.center();
        let idx = grid.idx(
            ((pt.x() - bounds.min_x) / resolution_m) as usize,
            ((pt.y() - bounds.min_y) / resolution_m) as usize,
        );
        // Don't add! If two buildings map to the same cell, we should pick a finer resolution.
        grid.data[idx] = cost.inner_seconds();
    }

    // Generate polygons covering the contour line where the cost in the grid crosses these
    // threshold values.
    let thresholds = vec![
        0.1,
        Duration::minutes(5).inner_seconds(),
        Duration::minutes(10).inner_seconds(),
        Duration::minutes(15).inner_seconds(),
    ];
    // And color the polygon for each threshold
    let colors = vec![
        Color::GREEN.alpha(0.5),
        Color::ORANGE.alpha(0.5),
        Color::RED.alpha(0.5),
    ];
    let smooth = false;
    let c = contour::ContourBuilder::new(grid.width as u32, grid.height as u32, smooth);
    let mut batch = GeomBatch::new();
    // The last feature returned will be larger than the last threshold value. We don't want to
    // display that at all. zip() will omit this last pair, since colors.len() ==
    // thresholds.len() - 1.
    //
    // TODO Actually, this still isn't working. I think each polygon is everything > the
    // threshold, not everything between two thresholds?
    for (feature, color) in c
        .contours(&grid.data, &thresholds)
        .unwrap()
        .into_iter()
        .zip(colors)
    {
        match feature.geometry.unwrap().value {
            geojson::Value::MultiPolygon(polygons) => {
                for p in polygons {
                    batch.push(color, Polygon::from_geojson(&p).scale(resolution_m));
                }
            }
            _ => unreachable!(),
        }
    }

    batch
}
