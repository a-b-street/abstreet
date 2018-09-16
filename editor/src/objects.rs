use colors::ColorScheme;
use control::ControlMap;
use ezgui::Canvas;
use kml::ExtraShapeID;
use map_model::{BuildingID, BusStopID, IntersectionID, LaneID, Map, ParcelID, TurnID};
use render::{DrawMap, Renderable};
use sim::{CarID, PedestrianID, Sim};

#[derive(Clone, Copy, Hash, PartialEq, Eq, Debug)]
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
}

impl ID {
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
        }
    }

    // TODO Renderable has tooltip_lines... decide what goes where
    pub fn tooltip_lines(&self, map: &Map, draw_map: &DrawMap, sim: &Sim) -> Vec<String> {
        match *self {
            ID::Lane(id) => draw_map.get_l(id).tooltip_lines(map),
            ID::Building(id) => draw_map.get_b(id).tooltip_lines(map),
            ID::Car(id) => sim.car_tooltip(id),
            ID::Pedestrian(id) => sim.ped_tooltip(id),
            ID::Intersection(id) => vec![format!("{}", id)],
            ID::Turn(id) => map.get_t(id).tooltip_lines(map),
            ID::ExtraShape(id) => draw_map.get_es(id).tooltip_lines(map),
            ID::BusStop(id) => draw_map.get_bs(id).tooltip_lines(map),
            ID::Parcel(id) => vec![format!("{}", id)],
        }
    }
}

// For plugins and rendering. Not sure what module this should live in, here seems fine.
pub struct Ctx<'a> {
    pub cs: &'a ColorScheme,
    pub map: &'a Map,
    pub control_map: &'a ControlMap,
    pub canvas: &'a Canvas,
    pub sim: &'a Sim,
}
