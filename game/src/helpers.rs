use crate::render::ExtraShapeID;
use crate::ui::PerMapUI;
use abstutil::Timer;
use ezgui::Color;
use geom::Pt2D;
use map_model::{AreaID, BuildingID, BusStopID, IntersectionID, LaneID, RoadID, TurnID};
use serde_derive::{Deserialize, Serialize};
use sim::{AgentID, CarID, PedestrianID};
use std::collections::{BTreeMap, HashMap};

// Aside from Road, everything here can actually be selected.
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
        }
    }
}

pub struct ColorScheme {
    map: HashMap<String, Color>,

    // A subset of map
    modified: ModifiedColors,

    path: String,
}

#[derive(Serialize, Deserialize)]
struct ModifiedColors {
    map: BTreeMap<String, Color>,
}

impl ColorScheme {
    // TODO When we quit with this, it'll save and overwrite it... remember the name too
    pub fn load(path: String) -> ColorScheme {
        let modified: ModifiedColors = abstutil::read_json(path.clone(), &mut Timer::throwaway());
        let mut map: HashMap<String, Color> = default_colors();
        for (name, c) in &modified.map {
            map.insert(name.clone(), *c);
        }
        ColorScheme {
            map,
            modified,
            path,
        }
    }

    pub fn save(&self) {
        abstutil::write_json(self.path.clone(), &self.modified);
    }

    // Get, but specify the default inline. The default is extracted before compilation by a script
    // and used to generate default_colors().
    pub fn get_def(&self, name: &str, _default: Color) -> Color {
        self.map[name]
    }

    pub fn get(&self, name: &str) -> Color {
        self.map[name]
    }

    pub fn color_names(&self) -> Vec<String> {
        let mut names: Vec<String> = self.map.keys().map(|n| n.to_string()).collect();
        names.sort();
        names
    }

    pub fn override_color(&mut self, name: &str, value: Color) {
        self.modified.map.insert(name.to_string(), value);
        self.map.insert(name.to_string(), value);
    }

    pub fn get_modified(&self, name: &str) -> Option<Color> {
        self.modified.map.get(name).cloned()
    }

    pub fn reset_modified(&mut self, name: &str, orig: Option<Color>) {
        if let Some(c) = orig {
            self.modified.map.insert(name.to_string(), c);
            self.map.insert(name.to_string(), c);
        } else {
            self.modified.map.remove(name);
            // Restore the original default.
            self.map.insert(name.to_string(), default_colors()[name]);
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
        return Color::CYAN;
    }
    if idx % 5 == 1 {
        return Color::BLUE;
    }
    if idx % 5 == 2 {
        return Color::GREEN;
    }
    if idx % 5 == 3 {
        return Color::ORANGE;
    }
    Color::RED
}
