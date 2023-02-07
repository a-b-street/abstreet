use crate::ID;
use abstutil::{prettyprint_usize, Counter};
use collisions::{CollisionDataset, Severity};
use geom::{Circle, Distance, Duration, FindClosest, Time};
use widgetry::mapspace::{DummyID, World};
use widgetry::{
    Choice, Color, EventCtx, GeomBatch, GfxCtx, HorizontalAlignment, Line, Outcome, Panel, Slider,
    State, Text, TextExt, Toggle, VerticalAlignment, Widget,
};

use crate::app::{App, Transition};

pub struct CollisionsViewer {
    data: CollisionDataset,
    world: World<DummyID>,
    panel: Panel,
}

impl CollisionsViewer {
    pub fn new_state(ctx: &mut EventCtx, app: &App) -> Box<dyn State<App>> {
        let map = &app.primary.map;
        let data = ctx.loading_screen("load collision data", |_, timer| {
            let mut all: CollisionDataset =
                abstio::read_binary(map.get_city_name().input_path("collisions.bin"), timer);
            all.collisions.retain(|c| {
                map.get_boundary_polygon()
                    .contains_pt(c.location.to_pt(map.get_gps_bounds()))
            });
            all
        });

        let filters = Filters::new();
        let indices = filters.apply(&data);
        let count = indices.len();
        let world = aggregated(ctx, app, &data, indices);

        Box::new(CollisionsViewer {
            panel: Panel::new_builder(Widget::col(vec![
                Widget::row(vec![
                    Line("Collisions viewer").small_heading().into_widget(ctx),
                    ctx.style().btn_close_widget(ctx),
                ]),
                format!("{} collisions", prettyprint_usize(count))
                    .text_widget(ctx)
                    .named("count"),
                Filters::make_controls(ctx).named("controls"),
            ]))
            .aligned(HorizontalAlignment::Right, VerticalAlignment::Top)
            .build(ctx),
            data,
            world,
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

    fn make_controls(ctx: &mut EventCtx) -> Widget {
        Widget::col(vec![
            Toggle::choice(
                ctx,
                "individual / aggregated",
                "individual",
                "aggregated",
                None,
                false,
            ),
            Widget::row(vec![
                "Between:".text_widget(ctx).margin_right(20),
                Slider::area(ctx, 0.1 * ctx.canvas.window_width, 0.0, "time1"),
            ]),
            Widget::row(vec![
                "and:".text_widget(ctx).margin_right(20),
                Slider::area(ctx, 0.1 * ctx.canvas.window_width, 1.0, "time2"),
            ]),
            Widget::row(vec![
                "Severity:".text_widget(ctx).margin_right(20),
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

fn aggregated(
    ctx: &mut EventCtx,
    app: &App,
    data: &CollisionDataset,
    indices: Vec<usize>,
) -> World<DummyID> {
    let map = &app.primary.map;

    // Match each collision to the nearest road and intersection
    let mut closest: FindClosest<ID> = FindClosest::new();
    for i in map.all_intersections() {
        closest.add_polygon(ID::Intersection(i.id), &i.polygon);
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

    let mut world = World::new();
    let scale = &app.cs.good_to_bad_red;
    // Same scale for both roads and intersections
    let total = per_road.max().max(per_intersection.max());

    for (r, count) in per_road.consume() {
        world
            .add_unnamed()
            // TODO Moving a very small bit of logic from ColorNetwork::pct_roads here...
            .hitbox(map.get_r(r).get_thick_polygon())
            .draw_color(scale.eval(pct(count, total)))
            .hover_alpha(0.5)
            .tooltip(Text::from(format!(
                "{} collisions",
                prettyprint_usize(count)
            )))
            .build(ctx);
    }
    for (i, count) in per_intersection.consume() {
        world
            .add_unnamed()
            .hitbox(map.get_i(i).polygon.clone())
            .draw_color(scale.eval(pct(count, total)))
            .hover_alpha(0.5)
            .tooltip(Text::from(format!(
                "{} collisions",
                prettyprint_usize(count)
            )))
            .build(ctx);
    }

    world.draw_master_batch(
        ctx,
        GeomBatch::from(vec![(
            app.cs.fade_map_dark,
            map.get_boundary_polygon().clone(),
        )]),
    );
    world.initialize_hover(ctx);
    world
}

fn individual(
    ctx: &mut EventCtx,
    app: &App,
    data: &CollisionDataset,
    indices: Vec<usize>,
) -> World<DummyID> {
    let map = &app.primary.map;
    let mut world = World::new();

    for idx in indices {
        let collision = &data.collisions[idx];

        // TODO Multiple collisions can occur at exactly the same spot. Need to add support for
        // that in World -- the KML viewer is the example to follow.
        world
            .add_unnamed()
            .hitbox(
                Circle::new(
                    collision.location.to_pt(map.get_gps_bounds()),
                    Distance::meters(5.0),
                )
                .to_polygon(),
            )
            .draw_color(Color::RED)
            .hover_alpha(0.5)
            .tooltip(Text::from_multiline(vec![
                Line(format!(
                    "Time: {}",
                    (Time::START_OF_DAY + collision.time).ampm_tostring()
                )),
                Line(format!("Severity: {:?}", collision.severity)),
            ]))
            .build(ctx);
    }

    world.draw_master_batch(
        ctx,
        GeomBatch::from(vec![(
            app.cs.fade_map_dark,
            map.get_boundary_polygon().clone(),
        )]),
    );
    world.initialize_hover(ctx);
    world
}

impl State<App> for CollisionsViewer {
    fn event(&mut self, ctx: &mut EventCtx, app: &mut App) -> Transition {
        self.world.event(ctx);

        match self.panel.event(ctx) {
            Outcome::Clicked(x) => match x.as_ref() {
                "close" => {
                    return Transition::Pop;
                }
                _ => unreachable!(),
            },
            Outcome::Changed(_) => {
                let filters = Filters::from_controls(&self.panel);
                let indices = filters.apply(&self.data);
                let count = indices.len();
                self.world = if filters.show_individual {
                    individual(ctx, app, &self.data, indices)
                } else {
                    aggregated(ctx, app, &self.data, indices)
                };
                let count = format!("{} collisions", prettyprint_usize(count)).text_widget(ctx);
                self.panel.replace(ctx, "count", count);
            }
            _ => {}
        }

        Transition::Keep
    }

    fn draw(&self, g: &mut GfxCtx, _: &App) {
        self.world.draw(g);
        self.panel.draw(g);
    }
}

// TODO Refactor -- wasn't geom Percent supposed to help?
fn pct(value: usize, total: usize) -> f64 {
    if total == 0 {
        1.0
    } else {
        value as f64 / total as f64
    }
}
