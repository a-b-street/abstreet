use rand::seq::SliceRandom;
use rand::SeedableRng;
use rand_xorshift::XorShiftRng;

use geom::{Circle, Distance, Duration, FindClosest, Polygon};
use map_model::NORMAL_LANE_THICKNESS;
use sim::{TripEndpoint, TripMode};
use widgetry::{
    Color, Drawable, EventCtx, GeomBatch, GfxCtx, HorizontalAlignment, Key, Line, Outcome, Panel,
    State, Text, TextExt, VerticalAlignment, Widget,
};

use crate::app::{App, Transition};

pub struct RoutePlanner {
    // All of this manages the waypoint input
    input_panel: Panel,
    waypoints: Vec<Waypoint>,
    draw_waypoints: Drawable,
    hovering_on_waypt: Option<usize>,
    draw_hover: Drawable,
    // TODO Invariant not captured by these separate fields: when dragging is true,
    // hovering_on_waypt is fixed.
    dragging: bool,
    snap_to_endpts: FindClosest<TripEndpoint>,

    // Routing
    draw_route: Drawable,
    results_panel: Panel,
}

// TODO Maybe it's been a while and I've forgotten some UI patterns, but this is painfully manual.
// I think we need a draggable map-space thing.
struct Waypoint {
    // TODO Different colors would also be helpful
    order: char,
    at: TripEndpoint,
    label: String,
    geom: GeomBatch,
    hitbox: Polygon,
}

impl RoutePlanner {
    pub fn new_state(ctx: &mut EventCtx, app: &App) -> Box<dyn State<App>> {
        let map = &app.primary.map;
        let mut snap_to_endpts = FindClosest::new(map.get_bounds());
        for i in map.all_intersections() {
            if i.is_border() {
                snap_to_endpts.add(TripEndpoint::Border(i.id), i.polygon.points());
            }
        }
        for b in map.all_buildings() {
            snap_to_endpts.add(TripEndpoint::Bldg(b.id), b.polygon.points());
        }

        let mut rp = RoutePlanner {
            input_panel: Panel::empty(ctx),
            waypoints: Vec::new(),
            draw_waypoints: Drawable::empty(ctx),
            hovering_on_waypt: None,
            draw_hover: Drawable::empty(ctx),
            dragging: false,
            snap_to_endpts,

            draw_route: Drawable::empty(ctx),
            results_panel: Panel::empty(ctx),
        };
        rp.update_input_panel(ctx);
        rp.update_waypoints_drawable(ctx);
        rp.update_route(ctx, app);
        Box::new(rp)
    }

    fn update_input_panel(&mut self, ctx: &mut EventCtx) {
        let mut col = vec![Widget::row(vec![
            Line("Plan a route").small_heading().into_widget(ctx),
            ctx.style().btn_close_widget(ctx),
        ])];

        for (idx, waypt) in self.waypoints.iter().enumerate() {
            col.push(Widget::row(vec![
                format!("{}) {}", waypt.order, waypt.label).text_widget(ctx),
                // TODO Circular outline style?
                ctx.style()
                    .btn_outline
                    .text("X")
                    .build_widget(ctx, &format!("delete waypoint {}", idx)),
            ]));
        }
        col.push(
            ctx.style()
                .btn_outline
                .text("Add waypoint")
                .hotkey(Key::A)
                .build_def(ctx),
        );

        self.input_panel = Panel::new_builder(Widget::col(col))
            .aligned(HorizontalAlignment::Left, VerticalAlignment::Top)
            .build(ctx);
    }

    fn update_waypoints_drawable(&mut self, ctx: &mut EventCtx) {
        let mut batch = GeomBatch::new();
        for waypt in &self.waypoints {
            batch.append(waypt.geom.clone());
        }
        self.draw_waypoints = ctx.upload(batch);
    }

    fn make_new_waypt(&self, ctx: &mut EventCtx, app: &App) -> Waypoint {
        // Just pick a random place, then let the user drag the marker around
        // TODO Repeat if it matches an existing
        let at = TripEndpoint::Bldg(
            app.primary
                .map
                .all_buildings()
                .choose(&mut XorShiftRng::from_entropy())
                .unwrap()
                .id,
        );
        Waypoint::new(ctx, app, at, self.waypoints.len())
    }

    fn update_hover(&mut self, ctx: &EventCtx) {
        self.hovering_on_waypt = None;

        if let Some(pt) = ctx.canvas.get_cursor_in_map_space() {
            self.hovering_on_waypt = self
                .waypoints
                .iter()
                .position(|waypt| waypt.hitbox.contains_pt(pt));
        }

        let mut batch = GeomBatch::new();
        if let Some(idx) = self.hovering_on_waypt {
            batch.push(Color::BLUE.alpha(0.5), self.waypoints[idx].hitbox.clone());
        }
        self.draw_hover = ctx.upload(batch);
    }

    // Just use Option for early return
    fn update_dragging(&mut self, ctx: &mut EventCtx, app: &App) -> Option<()> {
        let pt = ctx.canvas.get_cursor_in_map_space()?;
        let (at, _) = self.snap_to_endpts.closest_pt(pt, Distance::meters(30.0))?;

        let idx = self.hovering_on_waypt.unwrap();
        if self.waypoints[idx].at != at {
            self.waypoints[idx] = Waypoint::new(ctx, app, at, idx);
            self.update_input_panel(ctx);
            self.update_waypoints_drawable(ctx);
            self.update_route(ctx, app);
        }

        let mut batch = GeomBatch::new();
        // Show where we're currently snapped
        batch.push(Color::BLUE.alpha(0.5), self.waypoints[idx].hitbox.clone());
        self.draw_hover = ctx.upload(batch);

        Some(())
    }

    fn update_route(&mut self, ctx: &mut EventCtx, app: &App) {
        let mut batch = GeomBatch::new();
        let map = &app.primary.map;

        let mut total_distance = Distance::ZERO;
        let mut total_time = Duration::ZERO;

        for pair in self.waypoints.windows(2) {
            if let Some((path, draw_path)) =
                TripEndpoint::path_req(pair[0].at, pair[1].at, TripMode::Bike, map)
                    .and_then(|req| map.pathfind(req).ok())
                    .and_then(|path| {
                        path.trace(&app.primary.map)
                            .map(|pl| (path, pl.make_polygons(5.0 * NORMAL_LANE_THICKNESS)))
                    })
            {
                batch.push(Color::CYAN, draw_path);
                total_distance += path.total_length();
                total_time += path.estimate_duration(map, Some(map_model::MAX_BIKE_SPEED));
            }
        }

        self.draw_route = ctx.upload(batch);

        self.results_panel = Panel::new_builder(Widget::col(vec![
            Line("Your route").small_heading().into_widget(ctx),
            Text::from_all(vec![
                Line("Distance: ").secondary(),
                Line(total_distance.to_string(&app.opts.units)),
            ])
            .into_widget(ctx),
            Text::from_all(vec![
                Line("Estimated time: ").secondary(),
                Line(total_time.to_string(&app.opts.units)),
            ])
            .into_widget(ctx),
        ]))
        .aligned(HorizontalAlignment::Right, VerticalAlignment::Top)
        .build(ctx);
    }
}

impl State<App> for RoutePlanner {
    fn event(&mut self, ctx: &mut EventCtx, app: &mut App) -> Transition {
        if self.dragging {
            if ctx.redo_mouseover() {
                self.update_dragging(ctx, app);
            }
            if ctx.input.left_mouse_button_released() {
                self.dragging = false;
                self.update_hover(ctx);
            }
        } else {
            if ctx.redo_mouseover() {
                self.update_hover(ctx);
            }

            if self.hovering_on_waypt.is_none() {
                ctx.canvas_movement();
            } else if let Some((_, dy)) = ctx.input.get_mouse_scroll() {
                // Zooming is OK, but can't start click and drag
                ctx.canvas.zoom(dy, ctx.canvas.get_cursor());
            }

            if self.hovering_on_waypt.is_some() && ctx.input.left_mouse_button_pressed() {
                self.dragging = true;
            }
        }

        if let Outcome::Clicked(x) = self.input_panel.event(ctx) {
            match x.as_ref() {
                "close" => {
                    return Transition::Pop;
                }
                "Add waypoint" => {
                    self.waypoints.push(self.make_new_waypt(ctx, app));
                    self.update_input_panel(ctx);
                    self.update_waypoints_drawable(ctx);
                    self.update_route(ctx, app);
                }
                x => {
                    if let Some(x) = x.strip_prefix("delete waypoint ") {
                        let idx = x.parse::<usize>().unwrap();
                        self.waypoints.remove(idx);
                        // Recalculate labels, in case we deleted in the middle
                        for (idx, waypt) in self.waypoints.iter_mut().enumerate() {
                            *waypt = Waypoint::new(ctx, app, waypt.at, idx);
                        }

                        self.update_input_panel(ctx);
                        self.update_waypoints_drawable(ctx);
                        self.update_route(ctx, app);
                    } else {
                        unreachable!()
                    }
                }
            }
        }

        Transition::Keep
    }

    fn draw(&self, g: &mut GfxCtx, _: &App) {
        self.input_panel.draw(g);
        g.redraw(&self.draw_waypoints);
        g.redraw(&self.draw_hover);

        self.results_panel.draw(g);
        g.redraw(&self.draw_route);
    }
}

impl Waypoint {
    fn new(ctx: &mut EventCtx, app: &App, at: TripEndpoint, idx: usize) -> Waypoint {
        let order = char::from_u32('A' as u32 + idx as u32).unwrap();
        let map = &app.primary.map;
        let (center, label) = match at {
            TripEndpoint::Bldg(b) => {
                let b = map.get_b(b);
                (b.polygon.center(), b.address.clone())
            }
            TripEndpoint::Border(i) => {
                let i = map.get_i(i);
                (i.polygon.center(), i.name(app.opts.language.as_ref(), map))
            }
            TripEndpoint::SuddenlyAppear(pos) => (pos.pt(map), pos.to_string()),
        };
        let circle = Circle::new(center, Distance::meters(30.0)).to_polygon();
        let mut geom = GeomBatch::new();
        geom.push(Color::RED, circle.clone());
        geom.append(
            Text::from(Line(format!("{}", order)).fg(Color::WHITE))
                .render(ctx)
                .centered_on(center),
        );
        let hitbox = circle;

        Waypoint {
            order,
            at,
            label,
            geom,
            hitbox,
        }
    }
}
