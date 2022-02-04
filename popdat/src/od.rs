//! This is a standalone pipeline for generating a Scenario, starting from origin-destination data
//! (also called desire lines), which gives a count of commuters between two zones, breaking down
//! by mode.

use std::collections::HashMap;

use rand::seq::SliceRandom;
use rand::Rng;
use rand_xorshift::XorShiftRng;

use abstutil::{prettyprint_usize, Timer};
use geom::{Duration, Percent, PolyLine, Polygon, Pt2D, Time};
use map_model::{BuildingID, BuildingType, Map};
use synthpop::{IndividTrip, MapBorders, PersonSpec, TripEndpoint, TripMode, TripPurpose};

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
    pub include_zones: IncludeZonePolicy,
}

impl Options {
    pub fn default() -> Options {
        Options {
            departure_time: NormalDistribution::new(
                Duration::hours(8) + Duration::minutes(30),
                Duration::minutes(30),
            ),
            work_duration: NormalDistribution::new(Duration::hours(9), Duration::hours(1)),
            include_zones: IncludeZonePolicy::AllowRemote,
        }
    }
}

/// Only desire lines starting and ending in zones matching this policy will be used.
#[derive(PartialEq)]
pub enum IncludeZonePolicy {
    /// Keep zones that at least partially overlap the map's boundary. Note this doesn't mean no
    /// off-map trips will occur -- if a zone only partly overlaps the map, then some trips will
    /// snap to a border.
    MustOverlap,
    /// Keep all zones. When looking at desire lines between two remote zones, filter by those
    /// whose straight-line segment between zone centroids intersects the map boundary
    AllowRemote,
}

/// Generates a scenario from aggregated origin/destination data (DesireLines). The input describes
/// an exact number of people, who live in one zone and work in another (possibly the same) and
/// commute using some mode. For each of them, we just need to pick a specific home and workplace
/// from the zones, and use the Options to pick departure times. We'll wind up creating people who
/// just take two trips daily: home -> work -> home.
///
/// The home and workplace may be a specific building, or they're snapped to a map border,
/// resulting in trips that begin and/or end off-map. The amount of the zone that overlaps with the
/// map boundary determines this. If the zone and map boundary overlap 50% by area, then half of
/// the people to/from this zone will pick buildings, and half will pick borders.
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
    let zones = create_zones(map, zones, opts.include_zones, timer);

    let mut people = Vec::new();
    let mut on_map_only = 0;
    let mut lives_on_map = 0;
    let mut works_on_map = 0;
    let mut pass_through = 0;

    // TODO Temp for debugging
    let mut dump_polylines = Vec::new();
    let mut osrm_errors = 0;
    let osrm = crate::osrm::OSRM::new("http://127.0.0.1:5000".to_string());
    timer.start_iter("create people per desire line", desire_lines.len());
    for desire in desire_lines {
        timer.next();
        // Skip if we filtered out either zone.
        if !zones.contains_key(&desire.home_zone) || !zones.contains_key(&desire.work_zone) {
            continue;
        }

        let home_zone = &zones[&desire.home_zone];
        let work_zone = &zones[&desire.work_zone];

        // If both are remote, make sure the desire line intersects the map
        if home_zone.is_remote() && work_zone.is_remote() {
            if desire.home_zone == desire.work_zone {
                continue;
            }

            if !map
                .get_boundary_polygon()
                .intersects_polyline(&PolyLine::must_new(vec![
                    home_zone.center,
                    work_zone.center,
                ]))
            {
                continue;
            }
        }
        if dump_polylines.len() == 100 {
            continue;
        }

        match osrm.pathfind(map.get_gps_bounds(), home_zone.center, work_zone.center) {
            Ok(pl) => {
                dump_polylines.push(pl);
            }
            Err(_err) => {
                osrm_errors += 1;
            }
        }

        for _ in 0..desire.number_commuters {
            // Pick a specific home and workplace. It might be off-map, depending on how much the
            // zone overlaps the map.
            if let (Some((leave_home, goto_home)), Some((leave_work, goto_work))) = (
                home_zone.pick_home(desire.mode, map, rng),
                work_zone.pick_workplace(desire.mode, map, rng),
            ) {
                // remove_weird_schedules would clean this up later, but simpler to skip upfront
                if leave_home == goto_work || leave_work == goto_home {
                    continue;
                }

                match (goto_home, goto_work) {
                    (TripEndpoint::Building(_), TripEndpoint::Building(_)) => {
                        on_map_only += 1;
                    }
                    (TripEndpoint::Building(_), TripEndpoint::Border(_)) => {
                        lives_on_map += 1;
                    }
                    (TripEndpoint::Border(_), TripEndpoint::Building(_)) => {
                        works_on_map += 1;
                    }
                    (TripEndpoint::Border(_), TripEndpoint::Border(_)) => {
                        pass_through += 1;
                    }
                    _ => unreachable!(),
                }

                // Create their schedule
                let goto_work_time = Time::START_OF_DAY + opts.departure_time.sample(rng);
                let return_home_time = goto_work_time + opts.work_duration.sample(rng);
                people.push(PersonSpec {
                    orig_id: None,
                    trips: vec![
                        IndividTrip::new(
                            goto_work_time,
                            TripPurpose::Work,
                            leave_home,
                            goto_work,
                            desire.mode,
                        ),
                        IndividTrip::new(
                            return_home_time,
                            TripPurpose::Home,
                            leave_work,
                            goto_home,
                            desire.mode,
                        ),
                    ],
                });
            }
        }
    }
    abstio::write_json("osrm.json".to_string(), &dump_polylines);
    info!("{} OSRM errors", prettyprint_usize(osrm_errors));
    let total = on_map_only + lives_on_map + works_on_map + pass_through;
    for (x, label) in [
        (on_map_only, "live and work on-map"),
        (lives_on_map, "live on-map, work remote"),
        (works_on_map, "live remote, work on-map"),
        (pass_through, "just pass through"),
    ] {
        info!(
            "{} people ({}) {}",
            prettyprint_usize(x),
            Percent::of(x, total),
            label
        );
    }

    people
}

struct Zone {
    polygon: Polygon,
    center: Pt2D,
    pct_overlap: f64,
    // For each building, have a value describing how many people live or work there. The exact
    // value doesn't matter; it's just a relative weighting. This way, we can use a weighted sample
    // and match more people to larger homes/stores.
    homes: Vec<(BuildingID, usize)>,
    workplaces: Vec<(BuildingID, usize)>,
    borders: MapBorders,
}

impl Zone {
    fn is_remote(&self) -> bool {
        self.pct_overlap == 0.0
    }
}

fn create_zones(
    map: &Map,
    input: HashMap<String, Polygon>,
    include_zones: IncludeZonePolicy,
    timer: &mut Timer,
) -> HashMap<String, Zone> {
    let all_borders = MapBorders::new(map);

    let mut normal_zones = HashMap::new();
    let mut remote_zones = HashMap::new();
    for (name, zone) in timer
        .parallelize(
            "create zones",
            input.into_iter().collect(),
            |(name, polygon)| {
                let mut overlapping_area = 0.0;
                for p in polygon.intersection(map.get_boundary_polygon()) {
                    overlapping_area += p.area();
                }
                // Sometimes this is slightly over 100%, because funky things happen with the polygon
                // intersection.
                let pct_overlap = (overlapping_area / polygon.area()).min(1.0);
                let is_remote = pct_overlap == 0.0;

                if is_remote && include_zones == IncludeZonePolicy::MustOverlap {
                    None
                } else {
                    // Multiple zones might all use the same border.
                    let center = polygon.center();
                    let mut borders = all_borders.clone();
                    // TODO For remote zones, we should at least prune for borders on the correct
                    // "side" of the map. Or we can let fast_dist later take care of it.
                    if !is_remote {
                        for list in vec![
                            &mut borders.incoming_walking,
                            &mut borders.incoming_driving,
                            &mut borders.incoming_biking,
                            &mut borders.outgoing_walking,
                            &mut borders.outgoing_driving,
                            &mut borders.outgoing_biking,
                        ] {
                            // If the zone partly overlaps, only keep borders physically in the
                            // zone polygon
                            // TODO If the intersection geometry happens to leak out of the map
                            // boundary a bit, this could be wrong!
                            list.retain(|border| polygon.contains_pt(border.pos));
                        }
                    }
                    Some((
                        name,
                        Zone {
                            polygon,
                            center,
                            pct_overlap,
                            homes: Vec::new(),
                            workplaces: Vec::new(),
                            borders,
                        },
                    ))
                }
            },
        )
        .into_iter()
        .flatten()
    {
        if zone.is_remote() {
            remote_zones.insert(name, zone);
        } else {
            normal_zones.insert(name, zone);
        }
    }

    info!(
        "{} zones partly in the map boundary, {} remote zones",
        prettyprint_usize(normal_zones.len()),
        prettyprint_usize(remote_zones.len())
    );

    // Match all buildings to a normal zone.
    timer.start_iter("assign buildings to zones", map.all_buildings().len());
    for b in map.all_buildings() {
        timer.next();
        let center = b.polygon.center();
        // We're assuming zones don't overlap each other, so just look for the first match.
        if let Some((_, zone)) = normal_zones
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

    normal_zones.extend(remote_zones);
    normal_zones
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
            return Some((TripEndpoint::Building(b), TripEndpoint::Building(b)));
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
            return Some((TripEndpoint::Building(b), TripEndpoint::Building(b)));
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

        let leave_i = incoming
            .choose_weighted(rng, |border| {
                (border.weight as f64) * self.center.fast_dist(border.pos).into_inner()
            })
            .ok()?
            .i;

        // If we can use the same border on the way back, prefer that.
        if outgoing.iter().any(|border| border.i == leave_i) {
            return Some((TripEndpoint::Border(leave_i), TripEndpoint::Border(leave_i)));
        }
        // Otherwise, we might have to use a separate border to re-enter. Prefer the one closest to
        // the first, to have a better chance of matching up divided one-ways.
        let leave_pt = map.get_i(leave_i).polygon.center();
        let goto_i = outgoing
            .iter()
            .min_by_key(|border| map.get_i(border.i).polygon.center().dist_to(leave_pt))?
            .i;
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
