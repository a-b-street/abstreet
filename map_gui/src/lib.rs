//! This crate contains common code for applications that draw and interact with a `Map`.

#[macro_use]
extern crate log;

use abstutil::Timer;
use geom::{Duration, Pt2D, Time};
use map_model::{AreaID, BuildingID, BusStopID, IntersectionID, LaneID, Map, ParkingLotID, RoadID};
use sim::{AgentID, CarID, PedestrianID, Sim};
use widgetry::{EventCtx, GfxCtx, State};

pub use self::simple_app::SimpleApp;
use crate::render::DrawOptions;
use colors::{ColorScheme, ColorSchemeChoice};
use options::Options;
use render::DrawMap;

pub mod colors;
pub mod load;
pub mod options;
pub mod render;
mod simple_app;
pub mod tools;

/// An application wishing to use the tools in this crate has to implement this on the struct that
/// implements `widgetry::SharedAppState`, so that the tools here can access the map. See
/// `SimpleApp` for an example implementation.
pub trait AppLike {
    fn map(&self) -> &Map;
    fn sim(&self) -> &Sim;
    fn cs(&self) -> &ColorScheme;
    fn mut_cs(&mut self) -> &mut ColorScheme;
    fn draw_map(&self) -> &DrawMap;
    fn mut_draw_map(&mut self) -> &mut DrawMap;
    fn opts(&self) -> &Options;
    fn mut_opts(&mut self) -> &mut Options;
    fn map_switched(&mut self, ctx: &mut EventCtx, map: Map, timer: &mut Timer);
    fn draw_with_opts(&self, g: &mut GfxCtx, opts: DrawOptions);
    /// Create a `widgetry::State` that warps to the given point.
    fn make_warper(
        &mut self,
        ctx: &EventCtx,
        pt: Pt2D,
        target_cam_zoom: Option<f64>,
        id: Option<ID>,
    ) -> Box<dyn State<Self>>
    where
        Self: Sized;

    // These two are needed to render traffic signals. Splitting them from sim() allows
    // applications that don't run a traffic sim to work.
    fn sim_time(&self) -> Time {
        self.sim().time()
    }
    fn current_stage_and_remaining_time(&self, id: IntersectionID) -> (usize, Duration) {
        self.sim().current_stage_and_remaining_time(id)
    }

    /// Change the color scheme. Idempotent. Return true if there was a change.
    fn change_color_scheme(&mut self, ctx: &mut EventCtx, cs: ColorSchemeChoice) -> bool {
        if self.opts().color_scheme == cs {
            return false;
        }
        self.mut_opts().color_scheme = cs;
        *self.mut_cs() = ColorScheme::new(ctx, self.opts().color_scheme);

        ctx.loading_screen("rerendering map colors", |ctx, timer| {
            *self.mut_draw_map() = DrawMap::new(ctx, self.map(), self.opts(), self.cs(), timer);
        });

        true
    }
}

#[derive(Clone, Hash, PartialEq, Eq, Debug, PartialOrd, Ord)]
pub enum ID {
    Road(RoadID),
    Lane(LaneID),
    Intersection(IntersectionID),
    Building(BuildingID),
    ParkingLot(ParkingLotID),
    Car(CarID),
    Pedestrian(PedestrianID),
    PedCrowd(Vec<PedestrianID>),
    BusStop(BusStopID),
    Area(AreaID),
}

impl ID {
    pub fn from_agent(id: AgentID) -> ID {
        match id {
            AgentID::Car(id) => ID::Car(id),
            AgentID::Pedestrian(id) => ID::Pedestrian(id),
            AgentID::BusPassenger(_, bus) => ID::Car(bus),
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

    pub fn as_intersection(&self) -> IntersectionID {
        match *self {
            ID::Intersection(i) => i,
            _ => panic!("Can't call as_intersection on {:?}", self),
        }
    }

    pub fn as_building(&self) -> BuildingID {
        match *self {
            ID::Building(b) => b,
            _ => panic!("Can't call as_building on {:?}", self),
        }
    }
}
