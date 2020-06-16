use crate::app::App;
use ezgui::{
    Btn, Color, Composite, Drawable, EventCtx, FancyColor, GeomBatch, GfxCtx, HorizontalAlignment,
    Line, LinearGradient, Outcome, Text, TextExt, VerticalAlignment, Widget,
};
use geom::{Circle, Distance, Line, Polygon, Pt2D};
use map_model::{BuildingID, BusStopID, IntersectionID, LaneID, Map, ParkingLotID, RoadID};
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
    parking_lots: HashMap<ParkingLotID, Color>,
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
            parking_lots: HashMap::new(),
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
            parking_lots: HashMap::new(),
        }
    }

    // If true, destruct this Colorer.
    pub fn event(&mut self, ctx: &mut EventCtx) -> bool {
        match self.legend.event(ctx) {
            Some(Outcome::Clicked(x)) if x == "close" => true,
            Some(Outcome::Clicked(_)) => unreachable!(),
            None => false,
        }
    }

    pub fn draw(&self, g: &mut GfxCtx, app: &App) {
        if g.canvas.cam_zoom < app.opts.min_zoom_for_detail {
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

    pub fn add_pl(&mut self, pl: ParkingLotID, color: Color) {
        self.parking_lots.insert(pl, color);
    }

    pub fn intersections_from_roads(&mut self, map: &Map) {
        for i in map.all_intersections() {
            if let Some(idx) = i
                .roads
                .iter()
                .filter_map(|r| {
                    self.roads
                        .get(r)
                        .and_then(|color| self.prioritized_colors.iter().position(|c| c == color))
                })
                .min()
            {
                self.add_i(i.id, self.prioritized_colors[idx]);
            }
        }
    }

    pub fn build(self, ctx: &mut EventCtx, app: &App) -> Colorer {
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
            unzoomed.push(color, map.get_r(r).get_thick_polygon(&map).unwrap());
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

        for (pl, color) in self.parking_lots {
            zoomed.push(color.alpha(0.4), map.get_pl(pl).polygon.clone());
            unzoomed.push(color, map.get_pl(pl).polygon.clone());
        }

        // Build the legend
        let mut col = vec![Widget::row(vec![
            Widget::draw_svg(ctx, "../data/system/assets/tools/layers.svg").margin_right(10),
            self.title.draw_text(ctx).centered_vert().margin_right(5),
            Btn::plaintext("X").build(ctx, "close", None).align_right(),
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
}

pub struct ColorLegend {}

impl ColorLegend {
    pub fn row<S: Into<String>>(ctx: &mut EventCtx, color: Color, label: S) -> Widget {
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
            Text::from(Line(label)).wrap_to_pct(ctx, 35).draw(ctx),
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

    pub fn gradient<I: Into<String>>(
        ctx: &mut EventCtx,
        colors: Vec<Color>,
        labels: Vec<I>,
    ) -> Widget {
        assert!(colors.len() >= 2);
        let width = 300.0;
        let n = colors.len();
        let mut batch = GeomBatch::new();
        let width_each = width / ((n - 1) as f64);
        batch.fancy_push(
            FancyColor::LinearGradient(LinearGradient {
                line: Line::new(Pt2D::new(0.0, 0.0), Pt2D::new(width, 0.0)),
                stops: colors
                    .into_iter()
                    .enumerate()
                    .map(|(idx, color)| ((idx as f64) / ((n - 1) as f64), color))
                    .collect(),
            }),
            Polygon::union_all(
                (0..n - 1)
                    .map(|i| {
                        Polygon::rectangle(width_each, 32.0).translate((i as f64) * width_each, 0.0)
                    })
                    .collect(),
            ),
        );
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

pub struct Scale;
impl Scale {
    pub fn diverging(low_color: Color, mid_color: Color, high_color: Color) -> DivergingScale {
        DivergingScale {
            low_color,
            mid_color,
            high_color,
            min: 0.0,
            avg: 0.5,
            max: 1.0,
            ignore: None,
        }
    }
}

pub struct DivergingScale {
    low_color: Color,
    mid_color: Color,
    high_color: Color,
    min: f64,
    avg: f64,
    max: f64,
    ignore: Option<(f64, f64)>,
}

impl DivergingScale {
    pub fn range(mut self, min: f64, max: f64) -> DivergingScale {
        assert!(min < max);
        self.min = min;
        self.avg = (min + max) / 2.0;
        self.max = max;
        self
    }

    pub fn ignore(mut self, from: f64, to: f64) -> DivergingScale {
        assert!(from < to);
        self.ignore = Some((from, to));
        self
    }

    pub fn eval(&self, value: f64) -> Option<Color> {
        let value = value.max(self.min).min(self.max);
        if let Some((from, to)) = self.ignore {
            if value >= from && value <= to {
                return None;
            }
        }
        if value <= self.avg {
            Some(
                self.low_color
                    .lerp(self.mid_color, (value - self.min) / (self.avg - self.min)),
            )
        } else {
            Some(
                self.mid_color
                    .lerp(self.high_color, (value - self.avg) / (self.max - self.avg)),
            )
        }
    }

    pub fn make_legend<I: Into<String>>(self, ctx: &mut EventCtx, labels: Vec<I>) -> Widget {
        ColorLegend::gradient(
            ctx,
            vec![self.low_color, self.mid_color, self.high_color],
            labels,
        )
    }
}
