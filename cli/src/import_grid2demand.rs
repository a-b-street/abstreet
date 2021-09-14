use anyhow::{anyhow, bail, Result};
use serde::Deserialize;

use abstutil::{prettyprint_usize, Timer};
use geom::{Duration, LonLat, Time};
use map_model::Map;
use sim::{ExternalPerson, ExternalTrip, ExternalTripEndpoint, Scenario, TripMode, TripPurpose};

pub fn run(csv_path: String, map: String) -> Result<()> {
    let mut timer = Timer::new("import grid2demand");
    timer.start("parse CSV");
    let people = parse_trips(csv_path)?;
    timer.stop("parse CSV");
    let map = Map::load_synchronously(map, &mut timer);

    let mut s = Scenario::empty(&map, "grid2demand");
    // Include all buses/trains
    s.only_seed_buses = None;
    let orig_num = people.len();
    let skip_problems = true;
    s.people = ExternalPerson::import(&map, people, skip_problems)?;
    // Always clean up people with no-op trips (going between the same buildings)
    s = s.remove_weird_schedules();
    println!(
        "Imported {}/{} people",
        prettyprint_usize(s.people.len()),
        prettyprint_usize(orig_num)
    );
    s.save();

    Ok(())
}

fn parse_trips(csv_path: String) -> Result<Vec<ExternalPerson>> {
    let mut people = Vec::new();
    for rec in csv::Reader::from_reader(std::fs::File::open(csv_path)?).deserialize() {
        let rec: Record = rec?;
        let mode = match rec.agent_type.as_ref() {
            "v" => TripMode::Drive,
            "b" => TripMode::Bike,
            "p" => TripMode::Walk,
            "t" => TripMode::Transit,
            x => bail!("Unknown agent_type {}", x),
        };
        let (origin, destination) = parse_linestring(&rec.geometry)
            .ok_or_else(|| anyhow!("didn't parse geometry {}", rec.geometry))?;
        let departure = parse_time(rec.departure_time)?;
        // For each row in the CSV file, create a person who takes a single trip from the origin to
        // the destination. They do not take a later trip to return home.
        people.push(ExternalPerson {
            trips: vec![ExternalTrip {
                departure,
                origin: ExternalTripEndpoint::Position(origin),
                destination: ExternalTripEndpoint::Position(destination),
                mode,
                purpose: TripPurpose::Work,
            }],
        });
    }
    Ok(people)
}

fn parse_linestring(input: &str) -> Option<(LonLat, LonLat)> {
    // Input is something like LINESTRING(-122.3245062 47.6456213,-122.3142029 47.6675654)
    let mut nums = Vec::new();
    for x in input
        .strip_prefix("LINESTRING(")?
        .strip_suffix(')')?
        .split(&[' ', ','][..])
    {
        nums.push(x.parse::<f64>().ok()?);
    }
    if nums.len() != 4 {
        return None;
    }
    Some((LonLat::new(nums[0], nums[1]), LonLat::new(nums[2], nums[3])))
}

fn parse_time(input: String) -> Result<Time> {
    // Input is HHMM, like 0730
    let hours = input[0..2].parse::<usize>()?;
    let mins = input[2..].parse::<usize>()?;
    Ok(Time::START_OF_DAY + Duration::hours(hours) + Duration::minutes(mins))
}

#[derive(Debug, Deserialize)]
struct Record {
    agent_type: String,
    geometry: String,
    departure_time: String,
}
