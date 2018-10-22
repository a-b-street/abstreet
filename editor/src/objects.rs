use colors::ColorScheme;
use control::ControlMap;
use ezgui::Canvas;
use geom::Pt2D;
use kml::ExtraShapeID;
use map_model::{AreaID, BuildingID, BusStopID, IntersectionID, LaneID, Map, ParcelID, TurnID};
use render::DrawMap;
use sim::{AgentID, CarID, PedestrianID, Sim, TripID};

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

    pub fn debug(&self, map: &Map, control_map: &ControlMap, sim: &mut Sim) {
        match *self {
            ID::Lane(id) => {
                map.get_l(id).dump_debug();
            }
            ID::Intersection(id) => {
                map.get_i(id).dump_debug();
                sim.debug_intersection(id, control_map);
            }
            ID::Turn(_) => {}
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
            ID::ExtraShape(_) => {}
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
            ID::BusStop(id) => map
                .maybe_get_bs(id)
                .map(|bs| map.get_l(id.sidewalk).dist_along(bs.dist_along).0),
            ID::Area(id) => map.maybe_get_a(id).map(|a| Pt2D::center(&a.points)),
            ID::Trip(id) => sim.get_canonical_point_for_trip(id, map),
        }
    }

    pub fn tooltip_lines(&self, map: &Map, draw_map: &DrawMap, sim: &Sim) -> Vec<String> {
        match *self {
            ID::Car(id) => sim.car_tooltip(id),
            ID::Pedestrian(id) => sim.ped_tooltip(id),
            x => draw_map.get_obj(x).tooltip_lines(map),
        }
    }
}

// For plugins and rendering. Not sure what module this should live in, here seems fine.
pub struct Ctx<'a> {
    pub cs: &'a ColorScheme,
    pub map: &'a Map,
    pub control_map: &'a ControlMap,
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
