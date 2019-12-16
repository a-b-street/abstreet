use crate::helpers::ID;
use crate::managed::{Composite, ManagedWidget};
use crate::render::{DrawOptions, MIN_ZOOM_FOR_DETAIL};
use crate::ui::{ShowEverything, UI};
use ezgui::{Color, Drawable, EventCtx, GeomBatch, GfxCtx, Line, ScreenPt, Text};
use geom::{Circle, Distance, Pt2D};
use map_model::{LaneID, Map, RoadID};
use sim::DontDrawAgents;
use std::collections::HashMap;

pub struct RoadColorerBuilder {
    header: Text,
    prioritized_colors: Vec<(&'static str, Color)>,
    zoomed_override_colors: HashMap<ID, Color>,
    roads: HashMap<RoadID, Color>,
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
            ui.draw(g, opts, &DontDrawAgents {}, &ShowEverything::new());
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
    pub fn new(header: Text, prioritized_colors: Vec<(&'static str, Color)>) -> RoadColorerBuilder {
        RoadColorerBuilder {
            header,
            prioritized_colors,
            zoomed_override_colors: HashMap::new(),
            roads: HashMap::new(),
        }
    }

    pub fn add(&mut self, l: LaneID, color: Color, map: &Map) {
        self.zoomed_override_colors.insert(ID::Lane(l), color);
        let r = map.get_parent(l).id;
        if let Some(existing) = self.roads.get(&r) {
            if self
                .prioritized_colors
                .iter()
                .position(|(_, c)| *c == color)
                < self
                    .prioritized_colors
                    .iter()
                    .position(|(_, c)| c == existing)
            {
                self.roads.insert(r, color);
            }
        } else {
            self.roads.insert(r, color);
        }
    }

    pub fn build(self, ctx: &EventCtx, map: &Map) -> RoadColorer {
        let mut batch = GeomBatch::new();
        for (r, color) in self.roads {
            batch.push(color, map.get_r(r).get_thick_polygon().unwrap());
        }
        RoadColorer {
            zoomed_override_colors: self.zoomed_override_colors,
            unzoomed: batch.upload(ctx),
            legend: ColorLegend::new(ctx, self.header, self.prioritized_colors),
        }
    }
}

pub struct ObjectColorerBuilder {
    header: Text,
    prioritized_colors: Vec<(&'static str, Color)>,
    zoomed_override_colors: HashMap<ID, Color>,
    roads: Vec<(RoadID, Color)>,
}

pub struct ObjectColorer {
    zoomed_override_colors: HashMap<ID, Color>,
    unzoomed: Drawable,
    legend: ColorLegend,
}

impl ObjectColorer {
    pub fn draw(&self, g: &mut GfxCtx, ui: &UI) {
        let mut opts = DrawOptions::new();
        if g.canvas.cam_zoom < MIN_ZOOM_FOR_DETAIL {
            ui.draw(g, opts, &DontDrawAgents {}, &ShowEverything::new());
            g.redraw(&self.unzoomed);
        } else {
            opts.override_colors = self.zoomed_override_colors.clone();
            ui.draw(g, opts, &ui.primary.sim, &ShowEverything::new());
        }

        self.legend.draw(g);
    }
}

impl ObjectColorerBuilder {
    pub fn new(
        header: Text,
        prioritized_colors: Vec<(&'static str, Color)>,
    ) -> ObjectColorerBuilder {
        ObjectColorerBuilder {
            header,
            prioritized_colors,
            zoomed_override_colors: HashMap::new(),
            roads: Vec::new(),
        }
    }

    pub fn add(&mut self, id: ID, color: Color) {
        if let ID::Road(r) = id {
            self.roads.push((r, color));
        } else {
            self.zoomed_override_colors.insert(id, color);
        }
    }

    pub fn build(mut self, ctx: &EventCtx, map: &Map) -> ObjectColorer {
        let mut batch = GeomBatch::new();
        for (id, color) in &self.zoomed_override_colors {
            let poly = match id {
                ID::Building(b) => map.get_b(*b).polygon.clone(),
                ID::Intersection(i) => map.get_i(*i).polygon.clone(),
                _ => unreachable!(),
            };
            batch.push(*color, poly);
        }
        for (r, color) in self.roads {
            batch.push(color, map.get_r(r).get_thick_polygon().unwrap());
            for l in map.get_r(r).all_lanes() {
                self.zoomed_override_colors.insert(ID::Lane(l), color);
            }
        }
        ObjectColorer {
            zoomed_override_colors: self.zoomed_override_colors,
            unzoomed: batch.upload(ctx),
            legend: ColorLegend::new(ctx, self.header, self.prioritized_colors),
        }
    }
}

pub struct ColorLegend {
    composite: Composite,
}

impl ColorLegend {
    pub fn new(ctx: &EventCtx, header: Text, rows: Vec<(&str, Color)>) -> ColorLegend {
        // TODO add a bg here and stop using prompt?
        let mut col = vec![ManagedWidget::draw_text(ctx, header)];

        let radius = 15.0;
        for (label, color) in rows {
            col.push(ManagedWidget::row(vec![
                ManagedWidget::draw_batch(
                    ctx,
                    GeomBatch::from(vec![(
                        color,
                        Circle::new(Pt2D::new(radius, radius), Distance::meters(radius))
                            .to_polygon(),
                    )]),
                ),
                ManagedWidget::draw_text(ctx, Text::from(Line(label))),
            ]));
        }
        let mut composite = Composite::minimal_size(
            ManagedWidget::col(col).bg(Color::grey(0.4)),
            ScreenPt::new(0.0, 150.0),
        );
        // Do this here, otherwise we have to be sure to call event()
        composite.recompute_layout(ctx, &mut HashMap::new());

        ColorLegend { composite }
    }

    pub fn draw(&self, g: &mut GfxCtx) {
        self.composite.draw(g);
    }
}
