mod debug_objects;
mod follow;
mod neighborhood_summary;
mod search;
mod show_activity;
mod show_owner;
mod show_route;
mod turn_cycler;
mod warp;

use abstutil::Timer;
use ezgui::{Color, GfxCtx};
use map_model::Map;
use objects::{Ctx, ID};
use plugins::{Plugin, PluginCtx};
use render::DrawMap;

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
                Box::new(follow::FollowState::new()),
                Box::new(debug_objects::DebugObjectsState::new()),
                Box::new(neighborhood_summary::NeighborhoodSummary::new(
                    map, draw_map, timer,
                )),
                Box::new(show_activity::ShowActivityState::new()),
                Box::new(show_owner::ShowOwnerState::new()),
                Box::new(show_route::ShowRouteState::new()),
                Box::new(turn_cycler::TurnCyclerState::new()),
            ],
        }
    }
}

impl Plugin for ViewMode {
    fn event(&mut self, mut ctx: PluginCtx) -> bool {
        if self.warp.is_some() {
            if self.warp.as_mut().unwrap().new_event(&mut ctx) {
                return true;
            } else {
                self.warp = None;
                return false;
            }
        } else if let Some(p) = warp::WarpState::new(&mut ctx) {
            self.warp = Some(Box::new(p));
            return true;
        }

        if self.search.is_some() {
            if self.search.as_mut().unwrap().new_event(&mut ctx) {
                if self.search.as_ref().unwrap().is_blocking() {
                    return true;
                }
            } else {
                self.search = None;
                return false;
            }
        } else if let Some(p) = search::SearchState::new(&mut ctx) {
            self.search = Some(p);
            return true;
        }

        for p in self.ambient_plugins.iter_mut() {
            p.ambient_event(&mut ctx);
        }
        false
    }

    fn draw(&self, g: &mut GfxCtx, mut ctx: Ctx) {
        // Always draw these, even when a blocking plugin is active.
        for p in &self.ambient_plugins {
            p.new_draw(g, &mut ctx);
        }

        if let Some(ref p) = self.warp {
            p.new_draw(g, &mut ctx);
        }
        if let Some(ref p) = self.search {
            p.new_draw(g, &mut ctx);
        }
    }

    fn color_for(&self, obj: ID, mut ctx: Ctx) -> Option<Color> {
        // warp doesn't implement color_for.

        if let Some(ref p) = self.search {
            if let Some(c) = p.new_color_for(obj, &mut ctx) {
                return Some(c);
            }
        }

        // First one arbitrarily wins.
        for p in &self.ambient_plugins {
            if let Some(c) = p.new_color_for(obj, &mut ctx) {
                return Some(c);
            }
        }
        None
    }
}
