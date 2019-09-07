use crate::helpers::ID;
use crate::render::{DrawOptions, MIN_ZOOM_FOR_DETAIL};
use crate::ui::{ShowEverything, UI};
use ezgui::{Color, Drawable, EventCtx, GeomBatch, GfxCtx, Line, ScreenPt, Text, LINE_HEIGHT};
use geom::{Distance, Polygon, Pt2D};
use map_model::{BuildingID, LaneID, Map, RoadID};
use std::collections::HashMap;

pub struct RoadColorerBuilder {
    prioritized_colors: Vec<Color>,
    zoomed_override_colors: HashMap<ID, Color>,
    roads: HashMap<RoadID, Color>,
    legend: ColorLegend,
}

pub struct RoadColorer {
    zoomed_override_colors: HashMap<ID, Color>,
    unzoomed: Drawable,
    legend: ColorLegend,
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

        self.legend.draw(g);
    }
}

impl RoadColorerBuilder {
    // Colors listed earlier override those listed later. This is used in unzoomed mode, when one
    // road has lanes of different colors.
    pub fn new(title: &str, prioritized_colors: Vec<(&str, Color)>) -> RoadColorerBuilder {
        RoadColorerBuilder {
            prioritized_colors: prioritized_colors.iter().map(|(_, c)| *c).collect(),
            zoomed_override_colors: HashMap::new(),
            roads: HashMap::new(),
            legend: ColorLegend::new(title, prioritized_colors),
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
            legend: self.legend,
        }
    }
}

pub struct BuildingColorerBuilder {
    zoomed_override_colors: HashMap<ID, Color>,
    legend: ColorLegend,
}

pub struct BuildingColorer {
    zoomed_override_colors: HashMap<ID, Color>,
    unzoomed: Drawable,
    legend: ColorLegend,
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

        self.legend.draw(g);
    }
}

impl BuildingColorerBuilder {
    pub fn new(title: &str, rows: Vec<(&str, Color)>) -> BuildingColorerBuilder {
        BuildingColorerBuilder {
            zoomed_override_colors: HashMap::new(),
            legend: ColorLegend::new(title, rows),
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
            legend: self.legend,
        }
    }
}

pub struct ColorLegend {
    title: String,
    rows: Vec<(String, Color)>,
}

impl ColorLegend {
    pub fn new(title: &str, rows: Vec<(&str, Color)>) -> ColorLegend {
        ColorLegend {
            title: title.to_string(),
            rows: rows
                .into_iter()
                .map(|(label, c)| (label.to_string(), c.alpha(1.0)))
                .collect(),
        }
    }

    pub fn draw(&self, g: &mut GfxCtx) {
        // TODO Want to draw a little rectangular box on each row, but how do we know positioning?
        // - v1: manually figure it out here with line height, padding, etc
        // - v2: be able to say something like "row: rectangle with width=30, height=80% of row.
        // then 10px spacing. then this text"
        // TODO Need to recalculate all this if the panel moves
        let mut txt = Text::prompt(&self.title);
        for (label, _) in &self.rows {
            txt.add(Line(label));
        }
        g.draw_text_at_screenspace_topleft(
            &txt,
            ScreenPt::new(
                50.0,
                g.canvas.window_height - (LINE_HEIGHT * ((self.rows.len() + 2) as f64)),
            ),
        );

        let mut batch = GeomBatch::new();
        // Hacky way to extend the text box's background a little...
        batch.push(
            Color::grey(0.2),
            Polygon::rectangle_topleft(
                Pt2D::new(
                    0.0,
                    g.canvas.window_height - (LINE_HEIGHT * ((self.rows.len() + 2) as f64)),
                ),
                Distance::meters(50.0),
                Distance::meters(LINE_HEIGHT * ((self.rows.len() + 2) as f64)),
            ),
        );
        let square_dims = 0.8 * LINE_HEIGHT;
        for (idx, (_, c)) in self.rows.iter().enumerate() {
            let offset_from_bottom = 1 + self.rows.len() - idx;
            batch.push(
                *c,
                Polygon::rectangle_topleft(
                    Pt2D::new(
                        20.0,
                        g.canvas.window_height - LINE_HEIGHT * (offset_from_bottom as f64)
                            + (LINE_HEIGHT - square_dims) / 2.0,
                    ),
                    Distance::meters(square_dims),
                    Distance::meters(square_dims),
                ),
            );
        }
        g.fork_screenspace();
        batch.draw(g);
        g.unfork();
    }
}
