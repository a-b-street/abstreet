use std::collections::{HashMap, HashSet};

use abstutil::MultiMap;
use connectivity::Spot;
use geom::Duration;
use map_gui::tools::draw_isochrone;
use map_model::{
    connectivity, AmenityType, BuildingID, BuildingType, IntersectionID, LaneType, Map, Path,
    PathConstraints, PathRequest,
};
use widgetry::{Color, Drawable, EventCtx};

use crate::App;

/// Represents the area reachable from a single building.
pub struct Isochrone {
    /// The center of the isochrone (can be multiple points)
    pub start: Vec<BuildingID>,
    /// The options used to generate this isochrone
    pub options: Options,
    /// Colored polygon contours, uploaded to the GPU and ready for drawing
    pub draw: Drawable,
    /// Thresholds used to draw the isochrone
    pub thresholds: Vec<f64>,
    /// Colors used to draw the isochrone
    pub colors: Vec<Color>,
    /// How far away is each building from the start?
    pub time_to_reach_building: HashMap<BuildingID, Duration>,
    /// Per category of amenity, what buildings have that?
    pub amenities_reachable: MultiMap<AmenityType, BuildingID>,
    /// How many people live in the returned area, according to estimates included in the map (from
    /// city-specific parcel data, guesses from census, or a guess based on OSM tags)
    pub population: usize,
    /// How many sreet parking spots are on the same road as any buildings returned.
    pub onstreet_parking_spots: usize,
}

/// The constraints on how we're moving.
#[derive(Clone)]
pub enum Options {
    Walking(connectivity::WalkingOptions),
    Biking,
}

impl Options {
    /// Calculate the quickest time to reach buildings across the map from any of the starting
    /// points, subject to the walking/biking settings configured in these Options.
    pub fn times_from(self, map: &Map, starts: Vec<Spot>) -> HashMap<BuildingID, Duration> {
        match self {
            Options::Walking(opts) => {
                connectivity::all_walking_costs_from(map, starts, Duration::minutes(15), opts)
            }
            Options::Biking => connectivity::all_vehicle_costs_from(
                map,
                starts,
                Duration::minutes(15),
                PathConstraints::Bike,
            ),
        }
    }
}

impl Isochrone {
    pub fn new(
        ctx: &mut EventCtx,
        app: &App,
        start: Vec<BuildingID>,
        options: Options,
    ) -> Isochrone {
        let spot_starts = start.iter().map(|b_id| Spot::Building(*b_id)).collect();
        let time_to_reach_building = options.clone().times_from(&app.map, spot_starts);

        let mut amenities_reachable = MultiMap::new();
        let mut population = 0;
        let mut all_roads = HashSet::new();
        for b in time_to_reach_building.keys() {
            let bldg = app.map.get_b(*b);
            for amenity in &bldg.amenities {
                if let Some(category) = AmenityType::categorize(&amenity.amenity_type) {
                    amenities_reachable.insert(category, bldg.id);
                }
            }
            match bldg.bldg_type {
                BuildingType::Residential { num_residents, .. }
                | BuildingType::ResidentialCommercial(num_residents, _) => {
                    population += num_residents;
                }
                _ => {}
            }
            all_roads.insert(app.map.get_l(bldg.sidewalk_pos.lane()).parent);
        }

        let mut onstreet_parking_spots = 0;
        for r in all_roads {
            let r = app.map.get_r(r);
            for l in &r.lanes {
                if l.lane_type == LaneType::Parking {
                    onstreet_parking_spots += l.number_parking_spots(app.map.get_config());
                }
            }
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

        let mut i = Isochrone {
            start,
            options,
            draw: Drawable::empty(ctx),
            thresholds,
            colors,
            time_to_reach_building,
            amenities_reachable,
            population,
            onstreet_parking_spots,
        };

        i.draw = draw_isochrone(
            &app.map,
            &i.time_to_reach_building,
            &i.thresholds,
            &i.colors,
        )
        .upload(ctx);
        i
    }

    pub fn path_to(&self, map: &Map, to: BuildingID) -> Option<Path> {
        // Don't draw paths to places far away
        if !self.time_to_reach_building.contains_key(&to) {
            return None;
        }

        let constraints = match self.options {
            Options::Walking(_) => PathConstraints::Pedestrian,
            Options::Biking => PathConstraints::Bike,
        };

        let all_paths = self.start.iter().map(|b_id| {
            map.pathfind(PathRequest::between_buildings(map, *b_id, to, constraints).unwrap())
                .ok()
                .unwrap()
        });

        all_paths.min_by_key(|path| path.total_length())
    }
}

/// Represents the area reachable from all intersections on the map border
pub struct BorderIsochrone {
    /// The center of the isochrone (can be multiple points)
    pub start: Vec<IntersectionID>,
    /// The options used to generate this isochrone
    pub options: Options,
    /// Colored polygon contours, uploaded to the GPU and ready for drawing
    pub draw: Drawable,
    /// Thresholds used to draw the isochrone
    pub thresholds: Vec<f64>,
    /// Colors used to draw the isochrone
    pub colors: Vec<Color>,
    /// How far away is each building from the start?
    pub time_to_reach_building: HashMap<BuildingID, Duration>,
}

impl BorderIsochrone {
    pub fn new(
        ctx: &mut EventCtx,
        app: &App,
        start: Vec<IntersectionID>,
        options: Options,
    ) -> BorderIsochrone {
        let spot_starts = start.iter().map(|i_id| Spot::Border(*i_id)).collect();
        let time_to_reach_building = options.clone().times_from(&app.map, spot_starts);

        // Generate a single polygon showing 15 minutes from the border
        let thresholds = vec![0.1, Duration::minutes(15).inner_seconds()];

        // Use one color for the entire polygon
        let colors = vec![Color::rgb(0, 0, 0).alpha(0.3)];

        let mut i = BorderIsochrone {
            start,
            options,
            draw: Drawable::empty(ctx),
            thresholds,
            colors,
            time_to_reach_building,
        };

        i.draw = draw_isochrone(
            &app.map,
            &i.time_to_reach_building,
            &i.thresholds,
            &i.colors,
        )
        .upload(ctx);
        i
    }
}
