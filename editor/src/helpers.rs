use crate::render::{DrawMap, ExtraShapeID};
use crate::ui::PerMapUI;
use abstutil;
use ezgui::{Color, GfxCtx, Text};
use geom::Pt2D;
use map_model::raw_data::StableRoadID;
use map_model::{AreaID, BuildingID, BusStopID, IntersectionID, LaneID, Map, RoadID, TurnID};
use serde_derive::{Deserialize, Serialize};
use sim::{AgentID, CarID, GetDrawAgents, PedestrianID, Sim, TripID};
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

    pub fn debug(&self, map: &Map, sim: &Sim, draw_map: &DrawMap) {
        match *self {
            ID::Road(id) => {
                map.get_r(id).dump_debug();
            }
            ID::Lane(id) => {
                map.get_l(id).dump_debug();
            }
            ID::Intersection(id) => {
                map.get_i(id).dump_debug();
                sim.debug_intersection(id, map);
            }
            ID::Turn(id) => {
                map.get_t(id).dump_debug();
            }
            ID::Building(id) => {
                map.get_b(id).dump_debug();
                let parked_cars = sim.get_parked_cars_by_owner(id);
                println!(
                    "{} parked cars are owned by {}: {:?}",
                    parked_cars.len(),
                    id,
                    parked_cars
                        .iter()
                        .map(|p| p.vehicle.id)
                        .collect::<Vec<CarID>>()
                );
            }
            ID::Car(id) => {
                sim.debug_car(id);
            }
            ID::Pedestrian(id) => {
                sim.debug_ped(id);
            }
            ID::ExtraShape(id) => {
                let es = draw_map.get_es(id);
                for (k, v) in &es.attributes {
                    println!("{} = {}", k, v);
                }
                println!("associated road: {:?}", es.road);
            }
            ID::BusStop(id) => {
                map.get_bs(id).dump_debug();
            }
            ID::Area(id) => {
                map.get_a(id).dump_debug();
            }
            ID::Trip(id) => {
                sim.debug_trip(id);
            }
        }
    }

    pub fn tooltip_lines(&self, g: &mut GfxCtx, ctx: &PerMapUI) -> Text {
        let (map, sim, draw_map) = (&ctx.map, &ctx.sim, &ctx.draw_map);
        let mut txt = Text::new();
        match *self {
            ID::Road(id) => {
                let r = map.get_r(id);
                txt.add_line(format!("{} (originally {}) is ", r.id, r.stable_id));
                txt.append(r.get_name(), Some(Color::CYAN));
                txt.add_line(format!("From OSM way {}", r.osm_way_id));
            }
            ID::Lane(id) => {
                let l = map.get_l(id);
                let r = map.get_r(l.parent);
                let i1 = map.get_source_intersection(id);
                let i2 = map.get_destination_intersection(id);

                txt.add_line(format!("{} is ", l.id));
                txt.append(r.get_name(), Some(Color::CYAN));
                txt.add_line(format!("From OSM way {}", r.osm_way_id));
                txt.add_line(format!(
                    "Parent {} (originally {}) points to {}",
                    r.id, r.stable_id, r.dst_i
                ));
                txt.add_line(format!(
                    "Lane goes from {} to {}",
                    i1.elevation, i2.elevation
                ));
                txt.add_line(format!(
                    "Lane is {} long, parent {} is {} long",
                    l.length(),
                    r.id,
                    r.center_pts.length()
                ));
                styled_kv(&mut txt, &r.osm_tags);
                if l.is_parking() {
                    txt.add_line(format!("Has {} parking spots", l.number_parking_spots()));
                }
            }
            ID::Intersection(id) => {
                txt.add_line(id.to_string());
                let i = map.get_i(id);
                txt.add_line(format!("Roads: {:?}", i.roads));
                txt.add_line(format!(
                    "Orig roads: {:?}",
                    i.roads
                        .iter()
                        .map(|r| map.get_r(*r).stable_id)
                        .collect::<Vec<StableRoadID>>()
                ));
                txt.add_line(format!("Originally {}", i.stable_id));
            }
            ID::Turn(id) => {
                let t = map.get_t(id);
                txt.add_line(format!("{}", id));
                txt.add_line(format!("{:?}", t.turn_type));
            }
            ID::Building(id) => {
                let b = map.get_b(id);
                txt.add_line(format!(
                    "Building #{:?} (from OSM way {})",
                    id, b.osm_way_id
                ));
                txt.add_line(format!(
                    "Dist along sidewalk: {}",
                    b.front_path.sidewalk.dist_along()
                ));
                if let Some(units) = b.num_residential_units {
                    txt.add_line(format!("{} residential units", units));
                }
                styled_kv(&mut txt, &b.osm_tags);
            }
            ID::Car(id) => {
                for line in sim.car_tooltip(id) {
                    txt.add_wrapped_line(&g.canvas, line);
                }
            }
            ID::Pedestrian(id) => {
                for line in sim.ped_tooltip(id) {
                    txt.add_wrapped_line(&g.canvas, line);
                }
            }
            ID::ExtraShape(id) => {
                styled_kv(&mut txt, &draw_map.get_es(id).attributes);
            }
            ID::BusStop(id) => {
                txt.add_line(id.to_string());
                for r in map.get_all_bus_routes() {
                    if r.stops.contains(&id) {
                        txt.add_line(format!("- Route {}", r.name));
                    }
                }
            }
            ID::Area(id) => {
                let a = map.get_a(id);
                txt.add_line(format!("{} (from OSM {})", id, a.osm_id));
                styled_kv(&mut txt, &a.osm_tags);
            }
            ID::Trip(_) => {}
        };
        txt
    }

    pub fn canonical_point(&self, primary: &PerMapUI) -> Option<Pt2D> {
        match *self {
            ID::Road(id) => primary.map.maybe_get_r(id).map(|r| r.center_pts.first_pt()),
            ID::Lane(id) => primary.map.maybe_get_l(id).map(|l| l.first_pt()),
            ID::Intersection(id) => primary.map.maybe_get_i(id).map(|i| i.point),
            ID::Turn(id) => primary.map.maybe_get_i(id.parent).map(|i| i.point),
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

fn styled_kv(txt: &mut Text, tags: &BTreeMap<String, String>) {
    for (k, v) in tags {
        txt.push(format!("[red:{}] = [cyan:{}]", k, v));
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
