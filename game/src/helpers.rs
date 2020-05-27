use crate::app::{App, PerMap};
use ezgui::{hotkey, Btn, Color, EventCtx, Key, Line, Text, TextSpan, Widget};
use geom::{Duration, Pt2D};
use map_model::{AreaID, BuildingID, BusStopID, IntersectionID, LaneID, RoadID, TurnID};
use sim::{AgentID, CarID, PedestrianID, TripMode, TripPhaseType};
use std::collections::BTreeSet;

// Aside from Road and Trip, everything here can actually be selected.
#[derive(Clone, Hash, PartialEq, Eq, Debug, PartialOrd, Ord)]
pub enum ID {
    Road(RoadID),
    Lane(LaneID),
    Intersection(IntersectionID),
    Turn(TurnID),
    Building(BuildingID),
    Car(CarID),
    Pedestrian(PedestrianID),
    PedCrowd(Vec<PedestrianID>),
    BusStop(BusStopID),
    Area(AreaID),
}

impl abstutil::Cloneable for ID {}

impl ID {
    pub fn from_agent(id: AgentID) -> ID {
        match id {
            AgentID::Car(id) => ID::Car(id),
            AgentID::Pedestrian(id) => ID::Pedestrian(id),
        }
    }

    pub fn agent_id(&self) -> Option<AgentID> {
        match *self {
            ID::Car(id) => Some(AgentID::Car(id)),
            ID::Pedestrian(id) => Some(AgentID::Pedestrian(id)),
            // PedCrowd doesn't map to a single agent.
            _ => None,
        }
    }

    pub fn canonical_point(&self, primary: &PerMap) -> Option<Pt2D> {
        match *self {
            ID::Road(id) => primary.map.maybe_get_r(id).map(|r| r.center_pts.first_pt()),
            ID::Lane(id) => primary.map.maybe_get_l(id).map(|l| l.first_pt()),
            ID::Intersection(id) => primary.map.maybe_get_i(id).map(|i| i.polygon.center()),
            ID::Turn(id) => primary
                .map
                .maybe_get_i(id.parent)
                .map(|i| i.polygon.center()),
            ID::Building(id) => primary.map.maybe_get_b(id).map(|b| b.polygon.center()),
            ID::Car(id) => primary
                .sim
                .canonical_pt_for_agent(AgentID::Car(id), &primary.map),
            ID::Pedestrian(id) => primary
                .sim
                .canonical_pt_for_agent(AgentID::Pedestrian(id), &primary.map),
            ID::PedCrowd(ref members) => primary
                .sim
                .canonical_pt_for_agent(AgentID::Pedestrian(members[0]), &primary.map),
            ID::BusStop(id) => primary
                .map
                .maybe_get_bs(id)
                .map(|bs| bs.sidewalk_pos.pt(&primary.map)),
            ID::Area(id) => primary.map.maybe_get_a(id).map(|a| a.polygon.center()),
        }
    }
}

pub fn list_names<F: Fn(TextSpan) -> TextSpan>(txt: &mut Text, styler: F, names: BTreeSet<String>) {
    let len = names.len();
    for (idx, n) in names.into_iter().enumerate() {
        if idx != 0 {
            if idx == len - 1 {
                if len == 2 {
                    txt.append(Line(" and "));
                } else {
                    txt.append(Line(", and "));
                }
            } else {
                txt.append(Line(", "));
            }
        }
        txt.append(styler(Line(n)));
    }
}

// TODO Associate this with maps, but somehow avoid reading the entire file when listing them.
pub fn nice_map_name(name: &str) -> &str {
    match name {
        "23rd" => "23rd Ave E corridor",
        "ballard" => "Ballard",
        "caphill" => "Capitol Hill",
        "downtown" => "Downtown Seattle",
        "huge_seattle" => "Seattle (entire area)",
        "intl_district" => "International District and I90",
        "lakeslice" => "Lake Washington corridor",
        "montlake" => "Montlake and Eastlake",
        "mt_baker" => "Mt Baker",
        "udistrict" => "Univeristy District",
        "west_seattle" => "West Seattle",
        // Outside Seattle
        "downtown_atx" => "Downtown Austin",
        "huge_austin" => "Austin (entire area)",
        _ => name,
    }
}

// Shorter is better
pub fn cmp_duration_shorter(after: Duration, before: Duration) -> Vec<TextSpan> {
    if after.epsilon_eq(before) {
        vec![Line("same")]
    } else if after < before {
        vec![
            Line((before - after).to_string()).fg(Color::GREEN),
            Line(" faster"),
        ]
    } else if after > before {
        vec![
            Line((after - before).to_string()).fg(Color::RED),
            Line(" slower"),
        ]
    } else {
        unreachable!()
    }
}

pub fn color_for_mode(app: &App, m: TripMode) -> Color {
    match m {
        TripMode::Walk => app.cs.unzoomed_pedestrian,
        TripMode::Bike => app.cs.unzoomed_bike,
        TripMode::Transit => app.cs.unzoomed_bus,
        TripMode::Drive => app.cs.unzoomed_car,
    }
}

pub fn color_for_trip_phase(app: &App, tpt: TripPhaseType) -> Color {
    match tpt {
        TripPhaseType::Driving => app.cs.unzoomed_car,
        TripPhaseType::Walking => app.cs.unzoomed_pedestrian,
        TripPhaseType::Biking => app.cs.bike_lane,
        TripPhaseType::Parking => app.cs.parking_trip,
        TripPhaseType::WaitingForBus(_, _) => app.cs.bus_layer,
        TripPhaseType::RidingBus(_, _, _) => app.cs.bus_lane,
        TripPhaseType::Aborted | TripPhaseType::Finished => unreachable!(),
        TripPhaseType::DelayedStart => Color::YELLOW,
        TripPhaseType::Remote => Color::PINK,
    }
}

pub fn amenity_type(a: &str) -> Option<&str> {
    if a == "supermarket" || a == "convenience" {
        Some("groceries")
    } else if a == "restaurant"
        || a == "cafe"
        || a == "fast_food"
        || a == "food_court"
        || a == "ice_cream"
        || a == "pastry"
        || a == "deli"
    {
        Some("food")
    } else if a == "pub" || a == "bar" || a == "nightclub" || a == "lounge" {
        Some("bar")
    } else if a == "doctors"
        || a == "dentist"
        || a == "clinic"
        || a == "pharmacy"
        || a == "chiropractor"
    {
        Some("medical")
    } else if a == "place_of_worship" {
        Some("church / temple")
    } else if a == "college" || a == "school" || a == "kindergarten" || a == "university" {
        Some("education")
    } else if a == "bank" || a == "post_office" || a == "atm" {
        Some("bank / post office")
    } else if a == "theatre"
        || a == "arts_centre"
        || a == "library"
        || a == "cinema"
        || a == "art_gallery"
    {
        Some("media")
    } else if a == "childcare" {
        Some("childcare")
    } else if a == "second_hand"
        || a == "clothes"
        || a == "furniture"
        || a == "shoes"
        || a == "department_store"
    {
        Some("shopping")
    } else {
        None
    }
}

// TODO Well, there goes the nice consolidation of stuff in BtnBuilder. :\
pub fn hotkey_btn<I: Into<String>>(ctx: &EventCtx, app: &App, label: I, key: Key) -> Widget {
    let label = label.into();
    let mut txt = Text::new();
    txt.append(Line(key.describe()).fg(ctx.style().hotkey_color));
    txt.append(Line(format!(" - {}", label)));
    Btn::text_bg(label, txt, app.cs.section_bg, app.cs.hovering).build_def(ctx, hotkey(key))
}
