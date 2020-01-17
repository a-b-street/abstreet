use crate::render::ExtraShapeID;
use crate::ui::PerMapUI;
use abstutil::Timer;
use ezgui::{Color, Line, Text, TextSpan};
use geom::Pt2D;
use map_model::{AreaID, BuildingID, BusStopID, IntersectionID, LaneID, RoadID, TurnID};
use serde_derive::{Deserialize, Serialize};
use sim::{AgentID, CarID, PedestrianID, TripID};
use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::fmt::Write;

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
    Trip(TripID),
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

    pub fn canonical_point(&self, primary: &PerMapUI) -> Option<Pt2D> {
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
            ID::Trip(id) => primary.sim.get_canonical_pt_per_trip(id, &primary.map).ok(),
        }
    }
}

pub struct ColorScheme(HashMap<String, Color>);

// Ideal for editing; values are (hex, alpha value).
#[derive(Serialize, Deserialize)]
struct OverrideColorScheme(BTreeMap<String, (String, f32)>);

impl ColorScheme {
    pub fn load(maybe_path: Option<String>) -> ColorScheme {
        let mut map: HashMap<String, Color> = default_colors();

        // TODO For now, regenerate this manually. If the build system could write in data/system/
        // that'd be great, but...
        if false {
            let mut copy = OverrideColorScheme(BTreeMap::new());
            for (name, c) in &map {
                if let Color::RGBA(r, g, b, a) = *c {
                    let hex = format!(
                        "#{:02X}{:02X}{:02X}",
                        (r * 255.0) as usize,
                        (g * 255.0) as usize,
                        (b * 255.0) as usize
                    );
                    copy.0.insert(name.clone(), (hex, a));
                }
            }
            abstutil::write_json("../data/system/override_colors.json".to_string(), &copy);
        }

        if let Some(path) = maybe_path {
            let overrides: OverrideColorScheme = abstutil::read_json(path, &mut Timer::throwaway());
            for (name, (hex, a)) in overrides.0 {
                map.insert(name, Color::hex(&hex).alpha(a));
            }
        }
        ColorScheme(map)
    }

    // Get, but specify the default inline. The default is extracted before compilation by a script
    // and used to generate default_colors().
    pub fn get_def(&self, name: &str, _default: Color) -> Color {
        self.0[name]
    }

    pub fn get(&self, name: &str) -> Color {
        if let Some(c) = self.0.get(name) {
            *c
        } else {
            panic!("Color {} undefined", name);
        }
    }
}

include!(concat!(env!("OUT_DIR"), "/init_colors.rs"));

pub fn rotating_color(idx: usize) -> Color {
    rotating_color_total(idx, 9)
}

pub fn rotating_color_total(idx: usize, total: usize) -> Color {
    if total > 9 {
        return rotating_color_total(idx, 9);
    }
    if total < 3 {
        return rotating_color_total(idx, 3);
    }

    // TODO Cache this
    // TODO This palette doesn't contrast well with other stuff
    let colors: Vec<Color> =
        colorbrewer::get_color_ramp(colorbrewer::Palette::YlOrBr, total as u32)
            .unwrap()
            .into_iter()
            .map(Color::hex)
            .collect();

    colors[idx % total]
}

pub fn rotating_color_map(idx: usize) -> Color {
    if idx % 5 == 0 {
        return Color::RED;
    }
    if idx % 5 == 1 {
        return Color::BLUE;
    }
    if idx % 5 == 2 {
        return Color::GREEN;
    }
    if idx % 5 == 3 {
        return Color::PURPLE;
    }
    Color::BLACK
}

pub fn rotating_color_agents(idx: usize) -> Color {
    if idx % 5 == 0 {
        return Color::hex("#5C45A0");
    }
    if idx % 5 == 1 {
        return Color::hex("#3E8BC3");
    }
    if idx % 5 == 2 {
        return Color::hex("#E1BA13");
    }
    if idx % 5 == 3 {
        return Color::hex("#96322F");
    }
    Color::hex("#00A27B")
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

// TODO Ideally use list_names, but this is hard to make work with word wrap
pub fn plain_list_names(names: BTreeSet<String>) -> String {
    let mut s = String::new();
    let len = names.len();
    for (idx, n) in names.into_iter().enumerate() {
        if idx != 0 {
            if idx == len - 1 {
                if len == 2 {
                    write!(s, " and ").unwrap();
                } else {
                    write!(s, ", and ").unwrap();
                }
            } else {
                write!(s, ", ").unwrap();
            }
        }
        write!(s, "{}", n).unwrap();
    }
    s
}
