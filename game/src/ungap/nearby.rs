use geom::Duration;
use map_gui::tools::draw_isochrone;
use map_model::{BuildingType, DirectedRoadID, LaneType, Map, PathConstraints};
use widgetry::{Color, Drawable, EventCtx};

use crate::app::App;

// TODO This is a more limited version of 15m's Isochrone.
pub struct Nearby {
    pub draw_buffer: Drawable,
    pub population: usize,
    pub total_amenities: usize,
}

impl Nearby {
    pub fn new(ctx: &mut EventCtx, app: &App) -> Nearby {
        // Deliberately tiny
        let threshold = Duration::minutes(1);

        let map = &app.primary.map;
        let time_to_reach_building = map_model::connectivity::all_vehicle_costs_from(
            map,
            bike_network_roads(map)
                .into_iter()
                .map(map_model::connectivity::Spot::DirectedRoad)
                .collect(),
            threshold,
            PathConstraints::Bike,
        );

        let mut population = 0;
        let mut total_amenities = 0;
        for b in time_to_reach_building.keys() {
            let bldg = map.get_b(*b);
            total_amenities += bldg.amenities.len();
            match bldg.bldg_type {
                BuildingType::Residential { num_residents, .. }
                | BuildingType::ResidentialCommercial(num_residents, _) => {
                    population += num_residents;
                }
                _ => {}
            }
        }

        let draw_buffer = draw_isochrone(
            map,
            &time_to_reach_building,
            &[0.1, threshold.inner_seconds()],
            &[Color::BLUE.alpha(0.5)],
        )
        .upload(ctx);

        Nearby {
            draw_buffer,
            population,
            total_amenities,
        }
    }
}

fn bike_network_roads(map: &Map) -> Vec<DirectedRoadID> {
    // TODO Repeating some render_network_layer logic
    let mut results = Vec::new();
    for r in map.all_roads() {
        if r.is_cycleway()
            || crate::ungap::layers::is_greenway(r)
            || r.lanes_ltr.iter().any(|(_, _, lt)| *lt == LaneType::Biking)
        {
            // Just start from both directions
            results.extend(r.id.both_directions());
        }
    }
    results
}
