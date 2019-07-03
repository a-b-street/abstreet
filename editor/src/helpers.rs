use crate::render::ExtraShapeID;
use crate::ui::PerMapUI;
use abstutil;
use ezgui::Color;
use geom::Pt2D;
use map_model::{AreaID, BuildingID, BusStopID, IntersectionID, LaneID, RoadID, TurnID};
use serde_derive::{Deserialize, Serialize};
use sim::{AgentID, CarID, GetDrawAgents, PedestrianID, TripID};
use std::collections::{BTreeMap, HashMap};
use std::io::Error;

#[derive(Clone, Copy, Hash, PartialEq, Eq, Debug, PartialOrd, Ord)]
pub enum ID {
    Road(RoadID),
    Lane(LaneID),
    Intersection(IntersectionID),
    Turn(TurnID),
    Building(BuildingID),
    Car(CarID),
    Pedestrian(PedestrianID),
    ExtraShape(ExtraShapeID),
    BusStop(BusStopID),
    Area(AreaID),
    Trip(TripID),
}

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
                .get_draw_car(id, &primary.map)
                .map(|c| c.body.last_pt()),
            ID::Pedestrian(id) => primary.sim.get_draw_ped(id, &primary.map).map(|p| p.pos),
            // TODO maybe_get_es
            ID::ExtraShape(id) => Some(primary.draw_map.get_es(id).center()),
            ID::BusStop(id) => primary
                .map
                .maybe_get_bs(id)
                .map(|bs| bs.sidewalk_pos.pt(&primary.map)),
            ID::Area(id) => primary.map.maybe_get_a(id).map(|a| a.polygon.center()),
            ID::Trip(id) => primary.sim.get_canonical_pt_per_trip(id, &primary.map),
        }
    }
}

pub struct ColorScheme {
    map: HashMap<String, Color>,

    // A subset of map
    modified: ModifiedColors,
}

#[derive(Serialize, Deserialize)]
struct ModifiedColors {
    map: BTreeMap<String, Color>,
}

impl ColorScheme {
    pub fn load() -> Result<ColorScheme, Error> {
        let modified: ModifiedColors = abstutil::read_json("../color_scheme.json")?;
        let mut map: HashMap<String, Color> = default_colors();
        for (name, c) in &modified.map {
            map.insert(name.clone(), *c);
        }

        Ok(ColorScheme { map, modified })
    }

    pub fn save(&self) {
        abstutil::write_json("../color_scheme.json", &self.modified)
            .expect("Saving color_scheme.json failed");
    }

    // Get, but specify the default inline. The default is extracted before compilation by a script
    // and used to generate default_colors().
    pub fn get_def(&self, name: &str, _default: Color) -> Color {
        self.map[name]
    }

    pub fn get(&self, name: &str) -> Color {
        self.map[name]
    }

    // Just for the color picker plugin, that's why the funky return value
    pub fn color_names(&self) -> Vec<(String, ())> {
        let mut names: Vec<(String, ())> = self.map.keys().map(|n| (n.clone(), ())).collect();
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
            .map(Color::from_hex)
            .collect();

    colors[idx % total]
}
