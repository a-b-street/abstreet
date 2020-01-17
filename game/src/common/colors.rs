use crate::render::MIN_ZOOM_FOR_DETAIL;
use crate::ui::UI;
use ezgui::{
    Color, Composite, Drawable, EventCtx, GeomBatch, GfxCtx, HorizontalAlignment, Line,
    ManagedWidget, Outcome, Text, VerticalAlignment,
};
use geom::{Circle, Distance, Pt2D};
use map_model::{BuildingID, BusStopID, IntersectionID, LaneID, Map, RoadID};
use std::collections::HashMap;

pub struct ColorerBuilder {
    header: Text,
    prioritized_colors: Vec<(&'static str, Color)>,
    lanes: HashMap<LaneID, Color>,
    roads: HashMap<RoadID, Color>,
    intersections: HashMap<IntersectionID, Color>,
    buildings: HashMap<BuildingID, Color>,
    bus_stops: HashMap<BusStopID, Color>,
}

pub struct Colorer {
    zoomed: Drawable,
    pub unzoomed: Drawable,
    legend: Composite,
}

impl Colorer {
    // If true, destruct this Colorer.
    pub fn event(&mut self, ctx: &mut EventCtx) -> bool {
        match self.legend.event(ctx) {
            Some(Outcome::Clicked(x)) if x == "X" => true,
            Some(Outcome::Clicked(_)) => unreachable!(),
            None => false,
        }
    }

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
            bus_stops: HashMap::new(),
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

    pub fn add_bs(&mut self, bs: BusStopID, color: Color) {
        self.bus_stops.insert(bs, color);
    }

    pub fn build(self, ctx: &mut EventCtx, ui: &UI) -> Colorer {
        let mut zoomed = GeomBatch::new();
        let mut unzoomed = GeomBatch::new();
        let map = &ui.primary.map;

        for (l, color) in self.lanes {
            zoomed.push(
                color.alpha(0.4),
                ui.primary.draw_map.get_l(l).polygon.clone(),
            );
        }
        for (r, color) in self.roads {
            unzoomed.push(color, map.get_r(r).get_thick_polygon().unwrap());
        }

        for (i, color) in self.intersections {
            zoomed.push(color.alpha(0.4), map.get_i(i).polygon.clone());
            unzoomed.push(color, map.get_i(i).polygon.clone());
        }
        for (b, color) in self.buildings {
            zoomed.push(color.alpha(0.4), map.get_b(b).polygon.clone());
            unzoomed.push(color, map.get_b(b).polygon.clone());
        }

        for (bs, color) in self.bus_stops {
            let pt = map.get_bs(bs).sidewalk_pos.pt(map);
            zoomed.push(
                color.alpha(0.4),
                Circle::new(pt, Distance::meters(5.0)).to_polygon(),
            );
            unzoomed.push(color, Circle::new(pt, Distance::meters(15.0)).to_polygon());
        }

        // Build the legend
        let mut col = vec![ManagedWidget::row(vec![
            ManagedWidget::draw_text(ctx, self.header),
            crate::managed::Composite::text_button(ctx, "X", None).align_right(),
        ])];
        for (label, color) in self.prioritized_colors {
            col.push(ColorLegend::row(ctx, color, label));
        }
        let legend = Composite::new(ManagedWidget::col(col).bg(Color::grey(0.4)))
            .aligned(HorizontalAlignment::Right, VerticalAlignment::Center)
            .build(ctx);

        Colorer {
            zoomed: zoomed.upload(ctx),
            unzoomed: unzoomed.upload(ctx),
            legend,
        }
    }
}

pub struct ColorLegend {}

impl ColorLegend {
    pub fn row<S: Into<String>>(ctx: &mut EventCtx, color: Color, label: S) -> ManagedWidget {
        let radius = 15.0;
        ManagedWidget::row(vec![
            ManagedWidget::draw_batch(
                ctx,
                GeomBatch::from(vec![(
                    color,
                    Circle::new(Pt2D::new(radius, radius), Distance::meters(radius)).to_polygon(),
                )]),
            ),
            ManagedWidget::draw_text(ctx, Text::from(Line(label))),
        ])
    }
}
