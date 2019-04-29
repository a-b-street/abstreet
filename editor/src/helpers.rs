use crate::render::{DrawMap, ExtraShapeID};
use crate::ui::PerMapUI;
use abstutil;
use abstutil::WeightedUsizeChoice;
use ezgui::{Color, GfxCtx, Text, WrappedWizard};
use geom::{Duration, Pt2D};
use map_model::raw_data::StableRoadID;
use map_model::{
    AreaID, BuildingID, BusStopID, IntersectionID, LaneID, Map, MapEdits, Neighborhood,
    NeighborhoodBuilder, RoadID, TurnID,
};
use serde_derive::{Deserialize, Serialize};
use sim::{
    ABTest, AgentID, CarID, GetDrawAgents, OriginDestination, PedestrianID, Scenario, Sim, TripID,
};
use std::collections::{BTreeMap, HashMap, HashSet};
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
                txt.append(
                    r.osm_tags
                        .get("name")
                        .unwrap_or(&"???".to_string())
                        .to_string(),
                    Some(Color::CYAN),
                );
                txt.add_line(format!("From OSM way {}", r.osm_way_id));
            }
            ID::Lane(id) => {
                let l = map.get_l(id);
                let r = map.get_r(l.parent);
                let i1 = map.get_source_intersection(id);
                let i2 = map.get_destination_intersection(id);

                txt.add_line(format!("{} is ", l.id));
                txt.append(
                    r.osm_tags
                        .get("name")
                        .unwrap_or(&"???".to_string())
                        .to_string(),
                    Some(Color::CYAN),
                );
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

    pub fn canonical_point(&self, map: &Map, sim: &Sim, draw_map: &DrawMap) -> Option<Pt2D> {
        match *self {
            ID::Road(id) => map
                .maybe_get_r(id)
                .map(|r| r.original_center_pts.first_pt()),
            ID::Lane(id) => map.maybe_get_l(id).map(|l| l.first_pt()),
            ID::Intersection(id) => map.maybe_get_i(id).map(|i| i.point),
            ID::Turn(id) => map.maybe_get_i(id.parent).map(|i| i.point),
            ID::Building(id) => map.maybe_get_b(id).map(|b| b.polygon.center()),
            ID::Car(id) => sim.get_draw_car(id, map).map(|c| c.body.last_pt()),
            ID::Pedestrian(id) => sim.get_draw_ped(id, map).map(|p| p.pos),
            // TODO maybe_get_es
            ID::ExtraShape(id) => Some(draw_map.get_es(id).center()),
            ID::BusStop(id) => map.maybe_get_bs(id).map(|bs| bs.sidewalk_pos.pt(map)),
            ID::Area(id) => map.maybe_get_a(id).map(|a| a.polygon.center()),
            ID::Trip(id) => sim.get_canonical_pt_per_trip(id, map),
        }
    }
}

pub struct RenderingHints {
    pub suppress_traffic_signal_details: Option<IntersectionID>,
    pub hide_turn_icons: HashSet<TurnID>,
}

// TODO move to render module
pub struct DrawCtx<'a> {
    pub cs: &'a ColorScheme,
    pub map: &'a Map,
    pub draw_map: &'a DrawMap,
    pub sim: &'a Sim,
    pub hints: &'a RenderingHints,
}

fn styled_kv(txt: &mut Text, tags: &BTreeMap<String, String>) {
    for (k, v) in tags {
        txt.add_styled_line(k.to_string(), Some(Color::RED), None, None);
        txt.append(" = ".to_string(), None);
        txt.append(v.to_string(), Some(Color::CYAN));
    }
}

pub fn choose_neighborhood(map: &Map, wizard: &mut WrappedWizard, query: &str) -> Option<String> {
    let map_name = map.get_name().to_string();
    let gps_bounds = map.get_gps_bounds().clone();
    // Load the full object, since we usually visualize the neighborhood when menuing over it
    wizard
        .choose_something_no_keys::<Neighborhood>(
            query,
            Box::new(move || Neighborhood::load_all(&map_name, &gps_bounds)),
        )
        .map(|(n, _)| n)
}

pub fn load_neighborhood_builder(
    map: &Map,
    wizard: &mut WrappedWizard,
    query: &str,
) -> Option<NeighborhoodBuilder> {
    let map_name = map.get_name().to_string();
    wizard
        .choose_something_no_keys::<NeighborhoodBuilder>(
            query,
            Box::new(move || abstutil::load_all_objects("neighborhoods", &map_name)),
        )
        .map(|(_, n)| n)
}

pub fn load_scenario(map: &Map, wizard: &mut WrappedWizard, query: &str) -> Option<Scenario> {
    let map_name = map.get_name().to_string();
    wizard
        .choose_something_no_keys::<Scenario>(
            query,
            Box::new(move || abstutil::load_all_objects("scenarios", &map_name)),
        )
        .map(|(_, s)| s)
}

pub fn choose_scenario(map: &Map, wizard: &mut WrappedWizard, query: &str) -> Option<String> {
    let map_name = map.get_name().to_string();
    wizard
        .choose_something_no_keys::<String>(
            query,
            Box::new(move || abstutil::list_all_objects("scenarios", &map_name)),
        )
        .map(|(n, _)| n)
}

pub fn choose_edits(map: &Map, wizard: &mut WrappedWizard, query: &str) -> Option<String> {
    let map_name = map.get_name().to_string();
    wizard
        .choose_something_no_keys::<String>(
            query,
            Box::new(move || {
                let mut list = abstutil::list_all_objects("edits", &map_name);
                list.push(("no_edits".to_string(), "no_edits".to_string()));
                list
            }),
        )
        .map(|(n, _)| n)
}

pub fn load_edits(map: &Map, wizard: &mut WrappedWizard, query: &str) -> Option<MapEdits> {
    // TODO Exclude current?
    let map_name = map.get_name().to_string();
    wizard
        .choose_something_no_keys::<MapEdits>(
            query,
            Box::new(move || {
                let mut list = abstutil::load_all_objects("edits", &map_name);
                list.push(("no_edits".to_string(), MapEdits::new(map_name.clone())));
                list
            }),
        )
        .map(|(_, e)| e)
}

pub fn load_ab_test(map: &Map, wizard: &mut WrappedWizard, query: &str) -> Option<ABTest> {
    let map_name = map.get_name().to_string();
    wizard
        .choose_something_no_keys::<ABTest>(
            query,
            Box::new(move || abstutil::load_all_objects("ab_tests", &map_name)),
        )
        .map(|(_, t)| t)
}

pub fn input_time(wizard: &mut WrappedWizard, query: &str) -> Option<Duration> {
    wizard.input_something(query, None, Box::new(|line| Duration::parse(&line)))
}

pub fn input_weighted_usize(
    wizard: &mut WrappedWizard,
    query: &str,
) -> Option<WeightedUsizeChoice> {
    wizard.input_something(
        query,
        None,
        Box::new(|line| WeightedUsizeChoice::parse(&line)),
    )
}

// TODO Validate the intersection exists? Let them pick it with the cursor?
pub fn choose_intersection(wizard: &mut WrappedWizard, query: &str) -> Option<IntersectionID> {
    wizard.input_something(
        query,
        None,
        Box::new(|line| usize::from_str_radix(&line, 10).ok().map(IntersectionID)),
    )
}

pub fn choose_origin_destination(
    map: &Map,
    wizard: &mut WrappedWizard,
    query: &str,
) -> Option<OriginDestination> {
    let neighborhood = "Neighborhood";
    let border = "Border intersection";
    if wizard.choose_string(query, vec![neighborhood, border])? == neighborhood {
        choose_neighborhood(map, wizard, query).map(OriginDestination::Neighborhood)
    } else {
        choose_intersection(wizard, query).map(OriginDestination::Border)
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
        let modified: ModifiedColors = abstutil::read_json("../color_scheme")?;
        let mut map: HashMap<String, Color> = default_colors();
        for (name, c) in &modified.map {
            map.insert(name.clone(), *c);
        }

        Ok(ColorScheme { map, modified })
    }

    pub fn save(&self) {
        abstutil::write_json("../color_scheme", &self.modified)
            .expect("Saving color_scheme failed");
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
