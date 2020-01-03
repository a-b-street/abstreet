use crate::render::MIN_ZOOM_FOR_DETAIL;
use crate::ui::UI;
use ezgui::{
    Color, Composite, Drawable, EventCtx, GeomBatch, GfxCtx, Line, ManagedWidget, ScreenPt, Text,
};
use geom::{Circle, Distance, Pt2D};
use map_model::{BuildingID, IntersectionID, LaneID, Map, RoadID};
use std::collections::HashMap;

pub struct ColorerBuilder {
    header: Text,
    prioritized_colors: Vec<(&'static str, Color)>,
    lanes: HashMap<LaneID, Color>,
    roads: HashMap<RoadID, Color>,
    intersections: HashMap<IntersectionID, Color>,
    buildings: HashMap<BuildingID, Color>,
}

pub struct Colorer {
    zoomed: Drawable,
    unzoomed: Drawable,
    legend: ColorLegend,
}

impl Colorer {
    pub fn draw(&self, g: &mut GfxCtx) {
        if g.canvas.cam_zoom < MIN_ZOOM_FOR_DETAIL {
            g.redraw(&self.unzoomed);
        } else {
            g.redraw(&self.zoomed);
        }

        self.legend.draw(g);
    }
}

impl ColorerBuilder {
    // Colors listed earlier override those listed later. This is used in unzoomed mode, when one
    // road has lanes of different colors.
    pub fn new(header: Text, prioritized_colors: Vec<(&'static str, Color)>) -> ColorerBuilder {
        ColorerBuilder {
            header,
            prioritized_colors,
            lanes: HashMap::new(),
            roads: HashMap::new(),
            intersections: HashMap::new(),
            buildings: HashMap::new(),
        }
    }

    pub fn add_l(&mut self, l: LaneID, color: Color, map: &Map) {
        self.lanes.insert(l, color);
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

    pub fn add_r(&mut self, r: RoadID, color: Color, map: &Map) {
        self.roads.insert(r, color);
        for l in map.get_r(r).all_lanes() {
            self.lanes.insert(l, color);
        }
    }

    pub fn add_i(&mut self, i: IntersectionID, color: Color) {
        self.intersections.insert(i, color);
    }

    pub fn add_b(&mut self, b: BuildingID, color: Color) {
        self.buildings.insert(b, color);
    }

    pub fn build(self, ctx: &EventCtx, ui: &UI) -> Colorer {
        let mut zoomed = GeomBatch::new();
        let mut unzoomed = GeomBatch::new();

        for (l, color) in self.lanes {
            zoomed.push(
                color.alpha(0.4),
                ui.primary.draw_map.get_l(l).polygon.clone(),
            );
        }
        for (r, color) in self.roads {
            unzoomed.push(color, ui.primary.map.get_r(r).get_thick_polygon().unwrap());
        }

        for (i, color) in self.intersections {
            zoomed.push(color.alpha(0.4), ui.primary.map.get_i(i).polygon.clone());
            unzoomed.push(color, ui.primary.map.get_i(i).polygon.clone());
        }
        for (b, color) in self.buildings {
            zoomed.push(color.alpha(0.4), ui.primary.map.get_b(b).polygon.clone());
            unzoomed.push(color, ui.primary.map.get_b(b).polygon.clone());
        }

        Colorer {
            zoomed: zoomed.upload(ctx),
            unzoomed: unzoomed.upload(ctx),
            legend: ColorLegend::new(ctx, self.header, self.prioritized_colors),
        }
    }
}

pub struct ColorLegend {
    composite: Composite,
}

impl ColorLegend {
    pub fn new(ctx: &EventCtx, header: Text, rows: Vec<(&str, Color)>) -> ColorLegend {
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
        ColorLegend {
            composite: Composite::minimal_size(
                ctx,
                ManagedWidget::col(col).bg(Color::grey(0.4)),
                ScreenPt::new(0.0, 150.0),
            ),
        }
    }

    pub fn draw(&self, g: &mut GfxCtx) {
        self.composite.draw(g);
    }
}
