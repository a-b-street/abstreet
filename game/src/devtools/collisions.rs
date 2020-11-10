use abstutil::{prettyprint_usize, Counter};
use collisions::{CollisionDataset, Severity};
use geom::{Circle, Distance, Duration, FindClosest};
use map_model::{IntersectionID, RoadID};
use widgetry::{
    Btn, Checkbox, Choice, Color, Drawable, EventCtx, GeomBatch, GfxCtx, HorizontalAlignment, Line,
    Outcome, Panel, Slider, State, TextExt, VerticalAlignment, Widget,
};

use crate::app::App;
use crate::common::ColorNetwork;
use crate::game::Transition;
use crate::helpers::ID;

pub struct CollisionsViewer {
    data: CollisionDataset,
    dataviz: Dataviz,
    panel: Panel,
}

impl CollisionsViewer {
    pub fn new(ctx: &mut EventCtx, app: &App) -> Box<dyn State<App>> {
        let map = &app.primary.map;
        let data = ctx.loading_screen("load collision data", |_, mut timer| {
            let mut all: CollisionDataset = abstutil::read_binary(
                abstutil::path(format!("input/{}/collisions.bin", map.get_city_name())),
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
        let dataviz = Dataviz::aggregated(ctx, app, &data, indices);

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
        hitboxes: Vec<(Circle, usize)>,
    },
    Aggregated {
        unzoomed: Drawable,
        zoomed: Drawable,
        per_road: Counter<RoadID>,
        per_intersection: Counter<IntersectionID>,
    },
}

impl Dataviz {
    fn aggregated(
        ctx: &mut EventCtx,
        app: &App,
        data: &CollisionDataset,
        indices: Vec<usize>,
    ) -> Dataviz {
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

        // Color roads and intersections using the counts
        let mut colorer = ColorNetwork::new(app);
        // TODO We should use some scale for both!
        colorer.pct_roads(per_road.clone(), &app.cs.good_to_bad_red);
        colorer.pct_intersections(per_intersection.clone(), &app.cs.good_to_bad_red);
        let (unzoomed, zoomed) = colorer.build(ctx);

        Dataviz::Aggregated {
            unzoomed,
            zoomed,
            per_road,
            per_intersection,
        }
    }

    fn individual(
        ctx: &mut EventCtx,
        app: &App,
        data: &CollisionDataset,
        indices: Vec<usize>,
    ) -> Dataviz {
        let mut hitboxes = Vec::new();
        let mut batch = GeomBatch::new();
        for idx in indices {
            let collision = &data.collisions[idx];
            let circle = Circle::new(
                collision.location.to_pt(app.primary.map.get_gps_bounds()),
                Distance::meters(5.0),
            );
            batch.push(Color::RED, circle.to_polygon());
            hitboxes.push((circle, idx));
        }
        Dataviz::Individual {
            hitboxes,
            draw_all_circles: ctx.upload(batch),
        }
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
            self.dataviz = if filters.show_individual {
                Dataviz::individual(ctx, app, &self.data, indices)
            } else {
                Dataviz::aggregated(ctx, app, &self.data, indices)
            };
            let count = format!("{} collisions", prettyprint_usize(count)).draw_text(ctx);
            self.panel.replace(ctx, "count", count);
        }

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
        self.panel.draw(g);
    }
}
