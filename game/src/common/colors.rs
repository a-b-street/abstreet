use crate::app::App;
use crate::render::MIN_ZOOM_FOR_DETAIL;
use ezgui::{
    Btn, Color, Composite, Drawable, EventCtx, GeomBatch, GfxCtx, HorizontalAlignment, Line,
    Outcome, Text, TextExt, VerticalAlignment, Widget,
};
use geom::{Circle, Distance, Polygon, Pt2D};
use map_model::{BuildingID, BusStopID, IntersectionID, LaneID, Map, RoadID};
use std::collections::HashMap;

pub struct ColorerBuilder {
    title: String,
    extra_info: Vec<String>,
    // First takes precedence
    prioritized_colors: Vec<Color>,
    legend: Vec<Widget>,
    lanes: HashMap<LaneID, Color>,
    roads: HashMap<RoadID, Color>,
    intersections: HashMap<IntersectionID, Color>,
    buildings: HashMap<BuildingID, Color>,
    bus_stops: HashMap<BusStopID, Color>,
}

pub struct Colorer {
    pub zoomed: Drawable,
    pub unzoomed: Drawable,
    pub legend: Composite,
}

impl Colorer {
    // Colors listed earlier override those listed later. This is used in unzoomed mode, when one
    // road has lanes of different colors.
    pub fn discrete<I: Into<String>>(
        ctx: &mut EventCtx,
        title: I,
        extra_info: Vec<String>,
        entries: Vec<(&'static str, Color)>,
    ) -> ColorerBuilder {
        let mut legend = Vec::new();
        let mut prioritized_colors = Vec::new();
        for (label, color) in entries {
            legend.push(ColorLegend::row(ctx, color, label));
            prioritized_colors.push(color);
        }

        ColorerBuilder {
            title: title.into(),
            extra_info,
            prioritized_colors,
            legend,
            lanes: HashMap::new(),
            roads: HashMap::new(),
            intersections: HashMap::new(),
            buildings: HashMap::new(),
            bus_stops: HashMap::new(),
        }
    }

    pub fn scaled<I: Into<String>>(
        ctx: &mut EventCtx,
        title: I,
        extra_info: Vec<String>,
        colors: Vec<Color>,
        labels: Vec<&str>,
    ) -> ColorerBuilder {
        let mut prioritized_colors = colors.clone();
        prioritized_colors.reverse();
        let legend = vec![ColorLegend::scale(ctx, colors, labels)];

        ColorerBuilder {
            title: title.into(),
            extra_info,
            prioritized_colors,
            legend,
            lanes: HashMap::new(),
            roads: HashMap::new(),
            intersections: HashMap::new(),
            buildings: HashMap::new(),
            bus_stops: HashMap::new(),
        }
    }

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
    pub fn add_l(&mut self, l: LaneID, color: Color, map: &Map) {
        self.lanes.insert(l, color);
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

    pub fn set_extra_info(&mut self, extra_info: Vec<String>) {
        self.extra_info = extra_info;
    }

    pub fn build_both(self, ctx: &mut EventCtx, app: &App) -> Colorer {
        let mut zoomed = GeomBatch::new();
        let mut unzoomed = GeomBatch::new();
        let map = &app.primary.map;

        unzoomed.push(app.cs.fade_map_dark, map.get_boundary_polygon().clone());

        for (l, color) in self.lanes {
            zoomed.push(
                color.alpha(0.4),
                app.primary.draw_map.get_l(l).polygon.clone(),
            );
        }
        for (r, color) in self.roads {
            unzoomed.push(
                color,
                map.get_r(r).get_thick_polygon(&app.primary.map).unwrap(),
            );
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
        let mut col = vec![Widget::row(vec![
            Widget::draw_svg(ctx, "../data/system/assets/tools/layers.svg").margin_right(10),
            self.title.draw_text(ctx).centered_vert().margin_right(5),
            Btn::plaintext("X").build_def(ctx, None).align_right(),
        ])];
        if !self.extra_info.is_empty() {
            let mut txt = Text::new();
            for line in self.extra_info {
                txt.add(Line(line).small());
            }
            col.push(txt.draw(ctx).margin_below(5));
        }
        col.extend(self.legend);
        let legend = Composite::new(Widget::col(col).bg(app.cs.panel_bg).padding(16))
            .aligned(HorizontalAlignment::Right, VerticalAlignment::Center)
            .build(ctx);

        Colorer {
            zoomed: zoomed.upload(ctx),
            unzoomed: unzoomed.upload(ctx),
            legend,
        }
    }

    pub fn build_zoomed(self, ctx: &mut EventCtx, app: &App) -> Drawable {
        self.build_both(ctx, app).zoomed
    }
    pub fn build_unzoomed(self, ctx: &mut EventCtx, app: &App) -> Colorer {
        let mut c = self.build_both(ctx, app);
        c.zoomed = GeomBatch::new().upload(ctx);
        c
    }
}

pub struct ColorLegend {}

impl ColorLegend {
    pub fn row<S: Into<String>>(ctx: &mut EventCtx, color: Color, label: S) -> Widget {
        // TODO This is a little specialized for info panels.
        let mut txt = Text::new();
        // TODO This is wider than the 0.35 of info panels, because add_wrapped is quite bad at max
        // char width right now.
        txt.add_wrapped(label.into(), 0.5 * ctx.canvas.window_width);

        let radius = 15.0;
        Widget::row(vec![
            Widget::draw_batch(
                ctx,
                GeomBatch::from(vec![(
                    color,
                    Circle::new(Pt2D::new(radius, radius), Distance::meters(radius)).to_polygon(),
                )]),
            )
            .margin(5)
            .centered_vert(),
            txt.draw(ctx),
        ])
    }

    pub fn scale<I: Into<String>>(
        ctx: &mut EventCtx,
        colors: Vec<Color>,
        labels: Vec<I>,
    ) -> Widget {
        assert_eq!(colors.len(), labels.len() - 1);
        let mut batch = GeomBatch::new();
        let mut x = 0.0;
        for color in colors {
            batch.push(color, Polygon::rectangle(64.0, 32.0).translate(x, 0.0));
            x += 64.0;
        }
        // Extra wrapping to make the labels stretch against just the scale, not everything else
        // TODO Long labels aren't nicely lined up with the boundaries between buckets
        Widget::row(vec![Widget::col(vec![
            Widget::draw_batch(ctx, batch),
            Widget::row(
                labels
                    .into_iter()
                    .map(|lbl| Line(lbl).small().draw(ctx))
                    .collect(),
            )
            .evenly_spaced(),
        ])])
    }
}
