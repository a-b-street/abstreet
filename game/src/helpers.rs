use crate::app::PerMap;
use crate::render::ExtraShapeID;
use abstutil::prettyprint_usize;
use ezgui::{Color, Line, Text, TextSpan};
use geom::{Duration, Pt2D};
use map_model::{AreaID, BuildingID, BusStopID, IntersectionID, LaneID, RoadID, TurnID};
use sim::{AgentID, CarID, PedestrianID};
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
    ExtraShape(ExtraShapeID),
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
            // TODO maybe_get_es
            ID::ExtraShape(id) => Some(primary.draw_map.get_es(id).center()),
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
        "montlake" => "Montlake and Eastlake",
        _ => name,
    }
}

// Shorter is better
pub fn cmp_duration_shorter(now: Duration, baseline: Duration) -> Vec<TextSpan> {
    if now.epsilon_eq(baseline) {
        vec![Line("same as baseline")]
    } else if now < baseline {
        vec![
            Line((baseline - now).to_string()).fg(Color::GREEN),
            Line(" faster"),
        ]
    } else if now > baseline {
        vec![
            Line((now - baseline).to_string()).fg(Color::RED),
            Line(" slower"),
        ]
    } else {
        unreachable!()
    }
}

// Fewer is better
pub fn cmp_count_fewer(now: usize, baseline: usize) -> TextSpan {
    if now < baseline {
        Line(format!("{} fewer", prettyprint_usize(baseline - now))).fg(Color::GREEN)
    } else if now > baseline {
        Line(format!("{} more", prettyprint_usize(now - baseline))).fg(Color::RED)
    } else {
        Line("same as baseline")
    }
}
