mod debug_objects;
mod follow;
mod neighborhood_summary;
mod search;
mod show_activity;
mod show_associated;
mod show_route;
mod turn_cycler;
mod warp;

use crate::objects::{Ctx, ID};
use crate::plugins::{Plugin, PluginCtx};
use crate::render::DrawMap;
use abstutil::Timer;
use ezgui::{Color, GfxCtx};
use map_model::Map;
use piston::input::Key;

pub struct ViewMode {
    warp: Option<Box<Plugin>>,
    search: Option<search::SearchState>,
    ambient_plugins: Vec<Box<Plugin>>,
}

impl ViewMode {
    pub fn new(map: &Map, draw_map: &DrawMap, timer: &mut Timer) -> ViewMode {
        ViewMode {
            warp: None,
            search: None,
            ambient_plugins: vec![
                Box::new(debug_objects::DebugObjectsState::new(Key::LCtrl, Key::D)),
                Box::new(follow::FollowState::new(Key::F)),
                Box::new(neighborhood_summary::NeighborhoodSummary::new(
                    Key::Z,
                    map,
                    draw_map,
                    timer,
                )),
                Box::new(show_activity::ShowActivityState::new(Key::A)),
                Box::new(show_associated::ShowAssociatedState::new()),
                Box::new(show_route::ShowRouteState::new(Key::R, Key::L)),
                Box::new(turn_cycler::TurnCyclerState::new(Key::Tab)),
            ],
        }
    }
}

impl Plugin for ViewMode {
    fn blocking_event(&mut self, ctx: &mut PluginCtx) -> bool {
        if self.warp.is_some() {
            if self.warp.as_mut().unwrap().blocking_event(ctx) {
                return true;
            } else {
                self.warp = None;
                return false;
            }
        } else if let Some(p) = warp::WarpState::new(Key::J, ctx) {
            self.warp = Some(Box::new(p));
            return true;
        }

        if self.search.is_some() {
            if self.search.as_mut().unwrap().blocking_event(ctx) {
                if self.search.as_ref().unwrap().is_blocking() {
                    return true;
                }
            } else {
                self.search = None;
                return false;
            }
        } else if let Some(p) = search::SearchState::new(Key::Slash, ctx) {
            self.search = Some(p);
            return true;
        }

        for p in self.ambient_plugins.iter_mut() {
            p.ambient_event(ctx);
        }
        false
    }

    fn draw(&self, g: &mut GfxCtx, ctx: &Ctx) {
        // Always draw these, even when a blocking plugin is active.
        for p in &self.ambient_plugins {
            p.draw(g, ctx);
        }

        if let Some(ref p) = self.warp {
            p.draw(g, ctx);
        }
        if let Some(ref p) = self.search {
            p.draw(g, ctx);
        }
    }

    fn color_for(&self, obj: ID, ctx: &Ctx) -> Option<Color> {
        // warp doesn't implement color_for.

        if let Some(ref p) = self.search {
            if let Some(c) = p.color_for(obj, ctx) {
                return Some(c);
            }
        }

        // First one arbitrarily wins.
        for p in &self.ambient_plugins {
            if let Some(c) = p.color_for(obj, ctx) {
                return Some(c);
            }
        }
        None
    }
}
