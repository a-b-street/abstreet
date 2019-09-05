use crate::helpers::ID;
use crate::render::{DrawOptions, MIN_ZOOM_FOR_DETAIL};
use crate::ui::{ShowEverything, UI};
use ezgui::{Color, Drawable, EventCtx, GeomBatch, GfxCtx};
use map_model::{BuildingID, LaneID, Map, RoadID};
use std::collections::HashMap;

pub struct RoadColorerBuilder {
    prioritized_colors: Vec<Color>,
    zoomed_override_colors: HashMap<ID, Color>,
    roads: HashMap<RoadID, Color>,
}

pub struct RoadColorer {
    zoomed_override_colors: HashMap<ID, Color>,
    unzoomed: Drawable,
}

impl RoadColorer {
    pub fn draw(&self, g: &mut GfxCtx, ui: &UI) {
        let mut opts = DrawOptions::new();
        if g.canvas.cam_zoom < MIN_ZOOM_FOR_DETAIL {
            ui.draw(g, opts, &ui.primary.sim, &ShowEverything::new());
            g.redraw(&self.unzoomed);
        } else {
            opts.override_colors = self.zoomed_override_colors.clone();
            ui.draw(g, opts, &ui.primary.sim, &ShowEverything::new());
        }
    }
}

impl RoadColorerBuilder {
    // Colors listed earlier override those listed later. This is used in unzoomed mode, when one
    // road has lanes of different colors.
    pub fn new(prioritized_colors: Vec<Color>) -> RoadColorerBuilder {
        RoadColorerBuilder {
            prioritized_colors,
            zoomed_override_colors: HashMap::new(),
            roads: HashMap::new(),
        }
    }

    pub fn add(&mut self, l: LaneID, color: Color, map: &Map) {
        self.zoomed_override_colors.insert(ID::Lane(l), color);
        let r = map.get_parent(l).id;
        if let Some(existing) = self.roads.get(&r) {
            if self.prioritized_colors.iter().position(|c| *c == color)
                < self.prioritized_colors.iter().position(|c| c == existing)
            {
                self.roads.insert(r, color);
            }
        } else {
            self.roads.insert(r, color);
        }
    }

    pub fn build(self, ctx: &mut EventCtx, map: &Map) -> RoadColorer {
        let mut batch = GeomBatch::new();
        for (r, color) in self.roads {
            batch.push(color, map.get_r(r).get_thick_polygon().unwrap());
        }
        RoadColorer {
            zoomed_override_colors: self.zoomed_override_colors,
            unzoomed: ctx.prerender.upload(batch),
        }
    }
}

pub struct BuildingColorerBuilder {
    zoomed_override_colors: HashMap<ID, Color>,
}

pub struct BuildingColorer {
    zoomed_override_colors: HashMap<ID, Color>,
    unzoomed: Drawable,
}

impl BuildingColorer {
    pub fn draw(&self, g: &mut GfxCtx, ui: &UI) {
        let mut opts = DrawOptions::new();
        if g.canvas.cam_zoom < MIN_ZOOM_FOR_DETAIL {
            ui.draw(g, opts, &ui.primary.sim, &ShowEverything::new());
            g.redraw(&self.unzoomed);
        } else {
            opts.override_colors = self.zoomed_override_colors.clone();
            ui.draw(g, opts, &ui.primary.sim, &ShowEverything::new());
        }
    }
}

impl BuildingColorerBuilder {
    pub fn new() -> BuildingColorerBuilder {
        BuildingColorerBuilder {
            zoomed_override_colors: HashMap::new(),
        }
    }

    pub fn add(&mut self, b: BuildingID, color: Color) {
        self.zoomed_override_colors.insert(ID::Building(b), color);
    }

    pub fn build(self, ctx: &mut EventCtx, map: &Map) -> BuildingColorer {
        let mut batch = GeomBatch::new();
        for (id, color) in &self.zoomed_override_colors {
            if let ID::Building(b) = id {
                batch.push(*color, map.get_b(*b).polygon.clone());
            } else {
                unreachable!()
            }
        }
        BuildingColorer {
            zoomed_override_colors: self.zoomed_override_colors,
            unzoomed: ctx.prerender.upload(batch),
        }
    }
}
