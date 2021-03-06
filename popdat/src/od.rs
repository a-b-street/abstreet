//! This is an alternative pipeline for generating a Scenario, starting from origin-destination
//! data (also called desire lines), which gives a count of commuters between two zones, breaking
//! down by mode.
//!
//! Maybe someday, we'll merge the two approaches, and make the first generate DesireLines as an
//! intermediate step.

use std::collections::HashMap;

use rand::seq::SliceRandom;
use rand::Rng;
use rand_xorshift::XorShiftRng;

use abstutil::Timer;
use geom::{Duration, Polygon, Time};
use map_model::{BuildingID, BuildingType, Map};
use sim::{IndividTrip, MapBorders, PersonSpec, TripEndpoint, TripMode, TripPurpose};

/// This describes some number of commuters living in some named zone, working in another (or the
/// same zone), and commuting using some mode.
#[derive(Debug)]
pub struct DesireLine {
    pub home_zone: String,
    pub work_zone: String,
    pub mode: TripMode,
    pub number_commuters: usize,
}

// TODO Percentage of taking a lunch trip, when to do it, how far to venture out, what mode to
// use...
pub struct Options {
    /// When should somebody depart from home to work?
    pub departure_time: NormalDistribution,
    /// How long should somebody work before returning home?
    pub work_duration: NormalDistribution,
}

impl Options {
    pub fn default() -> Options {
        Options {
            departure_time: NormalDistribution::new(
                Duration::hours(8) + Duration::minutes(30),
                Duration::minutes(30),
            ),
            work_duration: NormalDistribution::new(Duration::hours(9), Duration::hours(1)),
        }
    }
}

/// TODO Describe. In particular, how are polygons partly or fully outside the map's boundary
/// handled?
/// TODO Add an options struct to specify AM/PM time distribution, lunch trips, etc.
pub fn disaggregate(
    map: &Map,
    zones: HashMap<String, Polygon>,
    desire_lines: Vec<DesireLine>,
    opts: Options,
    rng: &mut XorShiftRng,
    timer: &mut Timer,
) -> Vec<PersonSpec> {
    // First decide which zones are relevant for our map. Match homes, shops, and border
    // intersections to each zone.
    timer.start("match zones");
    let zones = create_zones(map, zones);
    timer.stop("match zones");

    let mut people = Vec::new();
    timer.start("create people");
    for desire in desire_lines {
        // Skip if we filtered out either zone.
        if !zones.contains_key(&desire.home_zone) || !zones.contains_key(&desire.work_zone) {
            continue;
        }
        let home_zone = &zones[&desire.home_zone];
        let work_zone = &zones[&desire.work_zone];

        for _ in 0..desire.number_commuters {
            // Pick a specific home and workplace. It might be off-map, depending on how much the
            // zone overlaps the map.
            if let (Some((leave_home, goto_home)), Some((_, goto_work))) = (
                home_zone.pick_home(desire.mode, map, rng),
                work_zone.pick_workplace(desire.mode, map, rng),
            ) {
                // Create their schedule
                let goto_work_time = Time::START_OF_DAY + opts.departure_time.sample(rng);
                let return_home_time = goto_work_time + opts.work_duration.sample(rng);
                people.push(PersonSpec {
                    orig_id: None,
                    origin: leave_home,
                    trips: vec![
                        IndividTrip::new(goto_work_time, TripPurpose::Work, goto_work, desire.mode),
                        IndividTrip::new(
                            return_home_time,
                            TripPurpose::Home,
                            goto_home,
                            desire.mode,
                        ),
                    ],
                });
            }
        }
    }
    timer.stop("create people");
    people
}

struct Zone {
    polygon: Polygon,
    pct_overlap: f64,
    // For each building, have a value describing how many people live or work there. The exact
    // value doesn't matter; it's just a relative weighting. This way, we can use a weighted sample
    // and match more people to larger homes/stores.
    homes: Vec<(BuildingID, usize)>,
    workplaces: Vec<(BuildingID, usize)>,
    borders: MapBorders,
}

fn create_zones(map: &Map, input: HashMap<String, Polygon>) -> HashMap<String, Zone> {
    let all_borders = MapBorders::new(map);
    let mut zones = HashMap::new();
    for (name, polygon) in input {
        let mut overlapping_area = 0.0;
        for p in polygon.intersection(map.get_boundary_polygon()) {
            overlapping_area += p.area();
        }
        // Sometimes this is slightly over 100%, because funky things happen with the polygon
        // intersection.
        let pct_overlap = (overlapping_area / polygon.area()).min(1.0);

        // If the zone doesn't intersect our map at all, totally skip it.
        if pct_overlap == 0.0 {
            continue;
        }
        zones.insert(
            name,
            Zone {
                polygon,
                pct_overlap,
                homes: Vec::new(),
                workplaces: Vec::new(),
                borders: all_borders.clone(),
            },
        );
    }

    // Match all buildings to a zone.
    for b in map.all_buildings() {
        let center = b.polygon.center();
        // We're assuming zones don't overlap each other, so just look for the first match.
        if let Some((_, zone)) = zones
            .iter_mut()
            .find(|(_, z)| z.polygon.contains_pt(center))
        {
            match b.bldg_type {
                // The current heuristics for num_residents sometimes assign 0 people to a
                // building. We never want that, so just scale them all up.
                BuildingType::Residential { num_residents, .. } => {
                    zone.homes.push((b.id, num_residents + 1));
                }
                BuildingType::ResidentialCommercial(num_residents, _) => {
                    zone.homes.push((b.id, num_residents + 1));
                    // We know how many different stores are located in each building, according to
                    // OSM. A big mall might have 10 amenities, while standalone
                    // shops just have 1.
                    zone.workplaces.push((b.id, b.amenities.len()));
                }
                BuildingType::Commercial(_) => {
                    zone.workplaces.push((b.id, b.amenities.len()));
                }
                BuildingType::Empty => {}
            }
        }
    }

    // Match border intersections to a zone.
    for zone in zones.values_mut() {
        let polygon = zone.polygon.clone();
        for list in vec![
            &mut zone.borders.incoming_walking,
            &mut zone.borders.incoming_driving,
            &mut zone.borders.incoming_biking,
            &mut zone.borders.outgoing_walking,
            &mut zone.borders.outgoing_driving,
            &mut zone.borders.outgoing_biking,
        ] {
            list.retain(|(i, _)| polygon.contains_pt(map.get_i(*i).polygon.center()));
        }
    }

    zones
}

impl Zone {
    /// Returns endpoints to (leave home, goto home). These're usually the same, except in some
    /// cases of border trips using divided one-ways.
    fn pick_home(
        &self,
        mode: TripMode,
        map: &Map,
        rng: &mut XorShiftRng,
    ) -> Option<(TripEndpoint, TripEndpoint)> {
        if rng.gen_bool(self.pct_overlap) && !self.homes.is_empty() {
            let b = self.homes.choose_weighted(rng, |(_, n)| *n).unwrap().0;
            return Some((TripEndpoint::Bldg(b), TripEndpoint::Bldg(b)));
        }
        self.pick_borders(mode, map, rng)
    }

    /// Returns endpoints to (leave work, goto work). These're usually the same, except in some
    /// cases of border trips using divided one-ways.
    fn pick_workplace(
        &self,
        mode: TripMode,
        map: &Map,
        rng: &mut XorShiftRng,
    ) -> Option<(TripEndpoint, TripEndpoint)> {
        if rng.gen_bool(self.pct_overlap) && !self.workplaces.is_empty() {
            let b = self.workplaces.choose_weighted(rng, |(_, n)| *n).unwrap().0;
            return Some((TripEndpoint::Bldg(b), TripEndpoint::Bldg(b)));
        }
        self.pick_borders(mode, map, rng)
    }

    fn pick_borders(
        &self,
        mode: TripMode,
        map: &Map,
        rng: &mut XorShiftRng,
    ) -> Option<(TripEndpoint, TripEndpoint)> {
        let (incoming, outgoing) = self.borders.for_mode(mode);
        let leave_i = incoming.choose(rng)?.0;
        // If we can use the same border on the way back, prefer that.
        if outgoing.iter().any(|(i, _)| *i == leave_i) {
            return Some((TripEndpoint::Border(leave_i), TripEndpoint::Border(leave_i)));
        }
        // Otherwise, we might have to use a separate border to re-enter. Prefer the one closest to
        // the first, to have a better chance of matching up divided one-ways.
        let leave_pt = map.get_i(leave_i).polygon.center();
        let goto_i = outgoing
            .iter()
            .min_by_key(|(i, _)| map.get_i(*i).polygon.center().dist_to(leave_pt))?
            .0;
        Some((TripEndpoint::Border(leave_i), TripEndpoint::Border(goto_i)))
    }
}

/// A normal distribution of Durations.
pub struct NormalDistribution {
    pub mean: Duration,
    pub std_deviation: Duration,
}

impl NormalDistribution {
    pub fn new(mean: Duration, std_deviation: Duration) -> NormalDistribution {
        NormalDistribution {
            mean,
            std_deviation,
        }
    }

    pub fn sample(&self, rng: &mut XorShiftRng) -> Duration {
        use rand_distr::{Distribution, Normal};

        Duration::seconds(
            Normal::new(
                self.mean.inner_seconds(),
                self.std_deviation.inner_seconds(),
            )
            .unwrap()
            .sample(rng),
        )
    }
}
