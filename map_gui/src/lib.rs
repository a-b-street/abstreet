//! This crate contains common code for applications that draw and interact with a `Map`.

// Disable some noisy clippy warnings
#![allow(clippy::too_many_arguments, clippy::type_complexity)]
#![allow(clippy::new_without_default)]

#[macro_use]
extern crate anyhow;
#[macro_use]
extern crate log;

use abstutil::Timer;
use geom::{Duration, Pt2D, Time};
use map_model::{
    AreaID, BuildingID, IntersectionID, LaneID, Map, ParkingLotID, RoadID, TransitStopID,
};
use widgetry::{EventCtx, GfxCtx, State};

pub use self::simple_app::{SimpleApp, SimpleAppArgs};
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

    // These two are needed to render traffic signals. They only make sense when there is a
    // simulation
    fn sim_time(&self) -> Time;
    fn current_stage_and_remaining_time(&self, id: IntersectionID) -> (usize, Duration);

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
    TransitStop(TransitStopID),
    Area(AreaID),
}
