use abstutil::{prettyprint_usize, Counter};
use collisions::{CollisionDataset, Severity};
use geom::{Circle, Distance, Duration, FindClosest, Polygon, Time};
use map_gui::tools::ColorNetwork;
use map_gui::ID;
use widgetry::{
    Btn, Checkbox, Choice, Color, Drawable, EventCtx, GeomBatch, GfxCtx, HorizontalAlignment, Line,
    Outcome, Panel, Slider, State, Text, TextExt, VerticalAlignment, Widget,
};

use crate::app::{App, Transition};

pub struct CollisionsViewer {
    data: CollisionDataset,
    dataviz: Dataviz,
    tooltips: MapspaceTooltips,
    panel: Panel,
}

impl CollisionsViewer {
    pub fn new(ctx: &mut EventCtx, app: &App) -> Box<dyn State<App>> {
        let map = &app.primary.map;
        let data = ctx.loading_screen("load collision data", |_, mut timer| {
            let mut all: CollisionDataset = abstio::read_binary(
                abstio::path(format!("input/{}/collisions.bin", map.get_city_name())),
                &mut timer,
            );
            all.collisions.retain(|c| {
                map.get_boundary_polygon()
                    .contains_pt(c.location.to_pt(map.get_gps_bounds()))
            });
            all
        });

        let filters = Filters::new();
        let indices = filters.apply(&data);
        let count = indices.len();
        let (dataviz, tooltips) = Dataviz::aggregated(ctx, app, &data, indices);

        Box::new(CollisionsViewer {
            panel: Panel::new(Widget::col(vec![
                Widget::row(vec![
                    Line("Collisions viewer").small_heading().draw(ctx),
                    Btn::close(ctx),
                ]),
                format!("{} collisions", prettyprint_usize(count))
                    .draw_text(ctx)
                    .named("count"),
                Filters::to_controls(ctx).named("controls"),
            ]))
            .aligned(HorizontalAlignment::Right, VerticalAlignment::Top)
            .build(ctx),
            data,
            dataviz,
            tooltips,
        })
    }
}

#[derive(PartialEq)]
struct Filters {
    show_individual: bool,
    time_range: (Duration, Duration),
    severity: Option<Severity>,
}

impl Filters {
    fn new() -> Filters {
        Filters {
            show_individual: false,
            time_range: (Duration::ZERO, Duration::hours(24)),
            severity: None,
        }
    }

    /// Returns the indices of all matching collisions
    fn apply(&self, data: &CollisionDataset) -> Vec<usize> {
        let mut indices = Vec::new();
        for (idx, c) in data.collisions.iter().enumerate() {
            if c.time < self.time_range.0 || c.time > self.time_range.1 {
                continue;
            }
            if self.severity.map(|s| s != c.severity).unwrap_or(false) {
                continue;
            }
            indices.push(idx);
        }
        indices
    }

    fn to_controls(ctx: &mut EventCtx) -> Widget {
        Widget::col(vec![
            Checkbox::toggle(
                ctx,
                "individual / aggregated",
                "individual",
                "aggregated",
                None,
                false,
            ),
            Widget::row(vec![
                "Between:".draw_text(ctx).margin_right(20),
                Slider::area(ctx, 0.1 * ctx.canvas.window_width, 0.0).named("time1"),
            ]),
            Widget::row(vec![
                "and:".draw_text(ctx).margin_right(20),
                Slider::area(ctx, 0.1 * ctx.canvas.window_width, 1.0).named("time2"),
            ]),
            Widget::row(vec![
                "Severity:".draw_text(ctx).margin_right(20),
                Widget::dropdown(
                    ctx,
                    "severity",
                    None,
                    vec![
                        Choice::new("any", None),
                        Choice::new("slight", Some(Severity::Slight)),
                        Choice::new("serious", Some(Severity::Serious)),
                        Choice::new("fatal", Some(Severity::Fatal)),
                    ],
                ),
            ]),
        ])
    }

    fn from_controls(panel: &Panel) -> Filters {
        let end_of_day = Duration::hours(24);
        Filters {
            show_individual: panel.is_checked("individual / aggregated"),
            time_range: (
                end_of_day * panel.slider("time1").get_percent(),
                end_of_day * panel.slider("time2").get_percent(),
            ),
            severity: panel.dropdown_value("severity"),
        }
    }
}

enum Dataviz {
    Individual {
        draw_all_circles: Drawable,
    },
    Aggregated {
        unzoomed: Drawable,
        zoomed: Drawable,
    },
}

impl Dataviz {
    fn aggregated(
        ctx: &mut EventCtx,
        app: &App,
        data: &CollisionDataset,
        indices: Vec<usize>,
    ) -> (Dataviz, MapspaceTooltips) {
        let map = &app.primary.map;

        // Match each collision to the nearest road and intersection
        let mut closest: FindClosest<ID> = FindClosest::new(map.get_bounds());
        for i in map.all_intersections() {
            closest.add(ID::Intersection(i.id), i.polygon.points());
        }
        for r in map.all_roads() {
            closest.add(ID::Road(r.id), r.center_pts.points());
        }

        // How many collisions occurred at each road and intersection?
        let mut per_road = Counter::new();
        let mut per_intersection = Counter::new();
        let mut unsnapped = 0;
        for idx in indices {
            let collision = &data.collisions[idx];
            // Search up to 10m away
            if let Some((id, _)) = closest.closest_pt(
                collision.location.to_pt(map.get_gps_bounds()),
                Distance::meters(10.0),
            ) {
                match id {
                    ID::Road(r) => {
                        per_road.inc(r);
                    }
                    ID::Intersection(i) => {
                        per_intersection.inc(i);
                    }
                    _ => unreachable!(),
                }
            } else {
                unsnapped += 1;
            }
        }
        if unsnapped > 0 {
            warn!(
                "{} collisions weren't close enough to a road or intersection",
                prettyprint_usize(unsnapped)
            );
        }

        // TODO Is it strange to not use the built-in DrawMap mouseover stuff for this?
        let mut tooltips = Vec::new();
        for (r, cnt) in per_road.borrow() {
            tooltips.push((
                map.get_r(*r).get_thick_polygon(map),
                Text::from(Line(format!("{} collisions", prettyprint_usize(*cnt)))),
            ));
        }
        for (i, cnt) in per_intersection.borrow() {
            tooltips.push((
                map.get_i(*i).polygon.clone(),
                Text::from(Line(format!("{} collisions", prettyprint_usize(*cnt)))),
            ));
        }
        let tooltips = MapspaceTooltips::new(
            tooltips,
            Box::new(|poly| GeomBatch::from(vec![(Color::BLUE.alpha(0.5), poly.clone())])),
        );

        // Color roads and intersections using the counts
        let mut colorer = ColorNetwork::new(app);
        // TODO We should use some scale for both!
        colorer.pct_roads(per_road, &app.cs.good_to_bad_red);
        colorer.pct_intersections(per_intersection, &app.cs.good_to_bad_red);
        let (unzoomed, zoomed) = colorer.build(ctx);

        (Dataviz::Aggregated { unzoomed, zoomed }, tooltips)
    }

    fn individual(
        ctx: &mut EventCtx,
        app: &App,
        data: &CollisionDataset,
        indices: Vec<usize>,
    ) -> (Dataviz, MapspaceTooltips) {
        let mut batch = GeomBatch::new();
        let mut tooltips = Vec::new();
        for idx in indices {
            let collision = &data.collisions[idx];
            let circle = Circle::new(
                collision.location.to_pt(app.primary.map.get_gps_bounds()),
                Distance::meters(5.0),
            )
            .to_polygon();
            batch.push(Color::RED, circle.clone());
            // TODO Er, but multiple collisions can occur at exactly the same spot
            tooltips.push((
                circle,
                Text::from_multiline(vec![
                    Line(format!(
                        "Time: {}",
                        (Time::START_OF_DAY + collision.time).ampm_tostring()
                    )),
                    Line(format!("Severity: {:?}", collision.severity)),
                ]),
            ));
        }
        let tooltips = MapspaceTooltips::new(
            tooltips,
            Box::new(|poly| GeomBatch::from(vec![(Color::BLUE.alpha(0.5), poly.clone())])),
        );

        (
            Dataviz::Individual {
                draw_all_circles: ctx.upload(batch),
            },
            tooltips,
        )
    }
}

impl State<App> for CollisionsViewer {
    fn event(&mut self, ctx: &mut EventCtx, app: &mut App) -> Transition {
        ctx.canvas_movement();

        let old_filters = Filters::from_controls(&self.panel);
        match self.panel.event(ctx) {
            Outcome::Clicked(x) => match x.as_ref() {
                "close" => {
                    return Transition::Pop;
                }
                _ => unreachable!(),
            },
            _ => {}
        }
        // TODO Should fiddling with sliders produce Outcome::Changed?
        let filters = Filters::from_controls(&self.panel);
        if filters != old_filters {
            let indices = filters.apply(&self.data);
            let count = indices.len();
            let (dataviz, tooltips) = if filters.show_individual {
                Dataviz::individual(ctx, app, &self.data, indices)
            } else {
                Dataviz::aggregated(ctx, app, &self.data, indices)
            };
            self.dataviz = dataviz;
            self.tooltips = tooltips;
            let count = format!("{} collisions", prettyprint_usize(count)).draw_text(ctx);
            self.panel.replace(ctx, "count", count);
        }

        self.tooltips.event(ctx);

        Transition::Keep
    }

    fn draw(&self, g: &mut GfxCtx, app: &App) {
        match self.dataviz {
            Dataviz::Aggregated {
                ref unzoomed,
                ref zoomed,
                ..
            } => {
                if g.canvas.cam_zoom < app.opts.min_zoom_for_detail {
                    g.redraw(unzoomed);
                } else {
                    g.redraw(zoomed);
                }
            }
            Dataviz::Individual {
                ref draw_all_circles,
                ..
            } => {
                g.redraw(draw_all_circles);
            }
        }
        self.tooltips.draw(g);
        self.panel.draw(g);
    }
}

// TODO Apply this to a few more places, and if it works well, lift to widgetry

struct MapspaceTooltips {
    // TODO Quadtree
    tooltips: Vec<(Polygon, Text)>,
    hover: Box<dyn Fn(&Polygon) -> GeomBatch>,
    selected: Option<usize>,
}

impl MapspaceTooltips {
    pub fn new(
        tooltips: Vec<(Polygon, Text)>,
        hover: Box<dyn Fn(&Polygon) -> GeomBatch>,
    ) -> MapspaceTooltips {
        MapspaceTooltips {
            tooltips,
            hover,
            selected: None,
        }
    }

    fn event(&mut self, ctx: &mut EventCtx) {
        if ctx.redo_mouseover() {
            self.selected = None;
            if let Some(pt) = ctx.canvas.get_cursor_in_map_space() {
                self.selected = self.tooltips.iter().position(|(p, _)| p.contains_pt(pt));
            }
        }
    }

    fn draw(&self, g: &mut GfxCtx) {
        if let Some(idx) = self.selected {
            let (polygon, txt) = &self.tooltips[idx];
            // TODO Cache
            let draw = g.upload((self.hover)(polygon));
            g.redraw(&draw);
            g.draw_mouse_tooltip(txt.clone());
        }
    }
}
