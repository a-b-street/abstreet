use colors::ColorScheme;
use ezgui::Canvas;
use geom::Pt2D;
use map_model::{AreaID, BuildingID, BusStopID, IntersectionID, LaneID, Map, ParcelID, TurnID};
use render::{DrawMap, ExtraShapeID};
use sim::{AgentID, CarID, GetDrawAgents, PedestrianID, Sim, TripID};

#[derive(Clone, Copy, Hash, PartialEq, Eq, Debug, PartialOrd, Ord)]
pub enum ID {
    Lane(LaneID),
    Intersection(IntersectionID),
    Turn(TurnID),
    Building(BuildingID),
    Car(CarID),
    Pedestrian(PedestrianID),
    ExtraShape(ExtraShapeID),
    Parcel(ParcelID),
    BusStop(BusStopID),
    Area(AreaID),
    Trip(TripID),
}

impl ID {
    pub fn agent_id(&self) -> Option<AgentID> {
        match *self {
            ID::Car(id) => Some(AgentID::Car(id)),
            ID::Pedestrian(id) => Some(AgentID::Pedestrian(id)),
            _ => None,
        }
    }

    pub fn debug(&self, map: &Map, sim: &mut Sim, draw_map: &DrawMap) {
        match *self {
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
                    parked_cars.iter().map(|p| p.car).collect::<Vec<CarID>>()
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
            ID::Parcel(id) => {
                map.get_p(id).dump_debug();
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

    pub fn canonical_point(&self, map: &Map, sim: &Sim, draw_map: &DrawMap) -> Option<Pt2D> {
        match *self {
            ID::Lane(id) => map.maybe_get_l(id).map(|l| l.first_pt()),
            ID::Intersection(id) => map.maybe_get_i(id).map(|i| i.point),
            ID::Turn(id) => map.maybe_get_i(id.parent).map(|i| i.point),
            ID::Building(id) => map.maybe_get_b(id).map(|b| Pt2D::center(&b.points)),
            ID::Car(id) => sim.get_draw_car(id, map).map(|c| c.front),
            ID::Pedestrian(id) => sim.get_draw_ped(id, map).map(|p| p.pos),
            // TODO maybe_get_es
            ID::ExtraShape(id) => Some(draw_map.get_es(id).center()),
            ID::Parcel(id) => map.maybe_get_p(id).map(|p| Pt2D::center(&p.points)),
            ID::BusStop(id) => map.maybe_get_bs(id).map(|bs| bs.sidewalk_pos.pt(map)),
            ID::Area(id) => map.maybe_get_a(id).map(|a| Pt2D::center(&a.points)),
            ID::Trip(id) => sim.get_stats().canonical_pt_per_trip.get(&id).map(|pt| *pt),
        }
    }
}

// For plugins and rendering. Not sure what module this should live in, here seems fine.
pub struct Ctx<'a> {
    pub cs: &'a mut ColorScheme,
    pub map: &'a Map,
    pub draw_map: &'a DrawMap,
    pub canvas: &'a Canvas,
    pub sim: &'a Sim,
}

// TODO not the right module for this, totally temp

pub const ROOT_MENU: &str = "";
pub const DEBUG: &str = "Debug";
pub const DEBUG_EXTRA: &str = "Debug/Show extra";
pub const DEBUG_LAYERS: &str = "Debug/Show layers";
pub const EDIT_MAP: &str = "Edit map";
pub const SETTINGS: &str = "Settings";
pub const SIM: &str = "Sim";
pub const SIM_SETUP: &str = "Sim/Setup";
