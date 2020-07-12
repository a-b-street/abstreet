use crate::app::App;
use crate::game::{DrawBaselayer, State, Transition};
use crate::helpers::{
    checkbox_per_mode, cmp_duration_shorter, color_for_mode, color_for_trip_phase,
};
use crate::info::{OpenTrip, Tab};
use crate::sandbox::dashboards::DashTab;
use crate::sandbox::SandboxMode;
use abstutil::prettyprint_usize;
use ezgui::{
    Btn, Checkbox, Color, Composite, EventCtx, Filler, GeomBatch, GfxCtx, Line, Outcome,
    RewriteColor, ScreenDims, ScreenPt, Text, TextExt, Widget,
};
use geom::{Distance, Duration, Polygon, Pt2D, Time};
use sim::{TripEndpoint, TripID, TripMode};
use std::collections::{BTreeSet, HashMap};

const ROWS: usize = 8;

pub struct TripTable {
    composite: Composite,
    opts: Options,
}

struct Options {
    sort_by: SortBy,
    descending: bool,
    modes: BTreeSet<TripMode>,
    off_map_starts: bool,
    off_map_ends: bool,
    skip: usize,
}

impl Options {
    fn change(&mut self, value: SortBy) {
        self.skip = 0;
        if self.sort_by == value {
            self.descending = !self.descending;
        } else {
            self.sort_by = value;
            self.descending = true;
        }
    }
}

// TODO Is there a heterogenously typed table crate somewhere?
#[derive(Clone, Copy, PartialEq)]
enum SortBy {
    Departure,
    Duration,
    RelativeDuration,
    PercentChangeDuration,
    Waiting,
    PercentWaiting,
}

impl TripTable {
    pub fn new(ctx: &mut EventCtx, app: &App) -> Box<dyn State> {
        let opts = Options {
            sort_by: SortBy::PercentWaiting,
            descending: true,
            modes: TripMode::all().into_iter().collect(),
            off_map_starts: true,
            off_map_ends: true,
            skip: 0,
        };
        Box::new(TripTable {
            composite: make(ctx, app, &opts),
            opts,
        })
    }

    fn recalc(&mut self, ctx: &mut EventCtx, app: &App) {
        let mut new = make(ctx, app, &self.opts);
        new.restore(ctx, &self.composite);
        self.composite = new;
    }
}

impl State for TripTable {
    fn event(&mut self, ctx: &mut EventCtx, app: &mut App) -> Transition {
        match self.composite.event(ctx) {
            Some(Outcome::Clicked(x)) => match x.as_ref() {
                "Departure" => {
                    self.opts.change(SortBy::Departure);
                    self.recalc(ctx, app);
                }
                "Duration" => {
                    self.opts.change(SortBy::Duration);
                    self.recalc(ctx, app);
                }
                "Comparison" => {
                    self.opts.change(SortBy::RelativeDuration);
                    self.recalc(ctx, app);
                }
                "Normalized" => {
                    self.opts.change(SortBy::PercentChangeDuration);
                    self.recalc(ctx, app);
                }
                "Time spent waiting" => {
                    self.opts.change(SortBy::Waiting);
                    self.recalc(ctx, app);
                }
                "Percent waiting" => {
                    self.opts.change(SortBy::PercentWaiting);
                    self.recalc(ctx, app);
                }
                "previous trips" => {
                    self.opts.skip -= ROWS;
                    self.recalc(ctx, app);
                }
                "next trips" => {
                    self.opts.skip += ROWS;
                    self.recalc(ctx, app);
                }
                x => {
                    if let Ok(idx) = x.parse::<usize>() {
                        let trip = TripID(idx);
                        let person = app.primary.sim.trip_to_person(trip);
                        return Transition::PopWithData(Box::new(move |state, ctx, app| {
                            let sandbox = state.downcast_mut::<SandboxMode>().unwrap();
                            let mut actions = sandbox.contextual_actions();
                            sandbox.controls.common.as_mut().unwrap().launch_info_panel(
                                ctx,
                                app,
                                Tab::PersonTrips(person, OpenTrip::single(trip)),
                                &mut actions,
                            );
                        }));
                    }
                    return DashTab::TripTable.transition(ctx, app, x);
                }
            },
            None => {
                let mut modes = BTreeSet::new();
                for m in TripMode::all() {
                    if self.composite.is_checked(m.ongoing_verb()) {
                        modes.insert(m);
                    }
                }
                if modes != self.opts.modes {
                    self.opts.skip = 0;
                    self.opts.modes = modes;
                    self.recalc(ctx, app);
                }
                let off_map_starts = self.composite.is_checked("starting off-map");
                let off_map_ends = self.composite.is_checked("ending off-map");
                if self.opts.off_map_starts != off_map_starts
                    || self.opts.off_map_ends != off_map_ends
                {
                    self.opts.off_map_starts = off_map_starts;
                    self.opts.off_map_ends = off_map_ends;
                    self.opts.skip = 0;
                    self.recalc(ctx, app);
                }
            }
        };

        Transition::Keep
    }

    fn draw_baselayer(&self) -> DrawBaselayer {
        DrawBaselayer::Custom
    }

    fn draw(&self, g: &mut GfxCtx, app: &App) {
        g.clear(app.cs.grass);
        self.composite.draw(g);
        preview_trip(g, app, &self.composite);
    }
}

struct Entry {
    trip: TripID,
    mode: TripMode,
    departure: Time,
    duration_after: Duration,
    duration_before: Duration,
    waiting: Duration,
    percent_waiting: usize,
}

fn make(ctx: &mut EventCtx, app: &App, opts: &Options) -> Composite {
    // Only make one pass through prebaked data
    let trip_times_before = if app.has_prebaked().is_some() {
        let mut times = HashMap::new();
        for (_, id, maybe_mode, dt) in &app.prebaked().finished_trips {
            if maybe_mode.is_some() {
                times.insert(*id, *dt);
            }
        }
        Some(times)
    } else {
        None
    };

    // Gather raw data
    let mut data = Vec::new();
    let sim = &app.primary.sim;
    let mut aborted = 0;
    for (_, id, maybe_mode, duration_after) in &sim.get_analytics().finished_trips {
        let mode = if let Some(m) = maybe_mode {
            if !opts.modes.contains(m) {
                continue;
            }
            *m
        } else {
            aborted += 1;
            continue;
        };
        let (departure, start, end, _) = sim.trip_info(*id);
        if !opts.off_map_starts {
            if let TripEndpoint::Border(_, _) = start {
                continue;
            }
        }
        if !opts.off_map_ends {
            if let TripEndpoint::Border(_, _) = end {
                continue;
            }
        }

        let (_, waiting) = sim.finished_trip_time(*id).unwrap();
        let duration_before = if let Some(ref times) = trip_times_before {
            if let Some(dt) = times.get(id) {
                *dt
            } else {
                // Aborted
                aborted += 1;
                continue;
            }
        } else {
            Duration::ZERO
        };

        data.push(Entry {
            trip: *id,
            mode,
            departure,
            duration_after: *duration_after,
            duration_before,
            waiting,
            percent_waiting: (100.0 * waiting / *duration_after) as usize,
        });
    }

    // Sort
    match opts.sort_by {
        SortBy::Departure => data.sort_by_key(|x| x.departure),
        SortBy::Duration => data.sort_by_key(|x| x.duration_after),
        SortBy::RelativeDuration => data.sort_by_key(|x| x.duration_after - x.duration_before),
        SortBy::PercentChangeDuration => {
            data.sort_by_key(|x| (100.0 * (x.duration_after / x.duration_before)) as isize)
        }
        SortBy::Waiting => data.sort_by_key(|x| x.waiting),
        SortBy::PercentWaiting => data.sort_by_key(|x| x.percent_waiting),
    }
    if opts.descending {
        data.reverse();
    }
    let total_rows = data.len();

    // Render data
    let mut rows = Vec::new();
    for x in data.into_iter().skip(opts.skip).take(ROWS) {
        let mut row = vec![
            Text::from(Line(x.trip.0.to_string())).render_ctx(ctx),
            Text::from(Line(x.mode.ongoing_verb()).fg(color_for_mode(app, x.mode))).render_ctx(ctx),
            Text::from(Line(x.departure.ampm_tostring())).render_ctx(ctx),
            Text::from(Line(x.duration_after.to_string())).render_ctx(ctx),
        ];
        if app.has_prebaked().is_some() {
            row.push(
                Text::from_all(cmp_duration_shorter(x.duration_after, x.duration_before))
                    .render_ctx(ctx),
            );
            if x.duration_after == x.duration_before {
                row.push(Text::from(Line("same")).render_ctx(ctx));
            } else if x.duration_after < x.duration_before {
                row.push(
                    Text::from(Line(format!(
                        "{}% faster",
                        (100.0 * (1.0 - (x.duration_after / x.duration_before))) as usize
                    )))
                    .render_ctx(ctx),
                );
            } else {
                row.push(
                    Text::from(Line(format!(
                        "{}% slower ",
                        (100.0 * ((x.duration_after / x.duration_before) - 1.0)) as usize
                    )))
                    .render_ctx(ctx),
                );
            }
        }
        row.push(Text::from(Line(x.waiting.to_string())).render_ctx(ctx));
        row.push(Text::from(Line(format!("{}%", x.percent_waiting))).render_ctx(ctx));

        rows.push((x.trip.0.to_string(), row));
    }

    let btn = |value, name| {
        if opts.sort_by == value {
            Btn::text_bg2(format!(
                "{} {}",
                name,
                if opts.descending { "↓" } else { "↑" }
            ))
            .build(ctx, name, None)
        } else {
            Btn::text_bg2(name).build_def(ctx, None)
        }
    };
    let mut headers = vec![
        Line("Trip ID").draw(ctx),
        Line("Type").draw(ctx),
        btn(SortBy::Departure, "Departure"),
        btn(SortBy::Duration, "Duration"),
    ];
    if app.has_prebaked().is_some() {
        headers.push(btn(SortBy::RelativeDuration, "Comparison"));
        headers.push(btn(SortBy::PercentChangeDuration, "Normalized"));
    }
    headers.push(btn(SortBy::Waiting, "Time spent waiting"));
    headers.push(btn(SortBy::PercentWaiting, "Percent waiting"));

    let mut col = vec![DashTab::TripTable.picker(ctx, app)];
    col.push(checkbox_per_mode(ctx, app, &opts.modes));
    col.push(Widget::row(vec![
        Checkbox::text(ctx, "starting off-map", None, opts.off_map_starts),
        Checkbox::text(ctx, "ending off-map", None, opts.off_map_ends),
    ]));
    let (_, unfinished, _) = app.primary.sim.num_trips();
    col.push(
        Text::from_multiline(vec![
            Line(format!(
                "{} trips aborted due to simulation glitch",
                prettyprint_usize(aborted)
            )),
            Line(format!(
                "{} unfinished trips remaining",
                prettyprint_usize(unfinished)
            )),
        ])
        .draw(ctx),
    );

    col.push(Widget::row(vec![
        if opts.skip > 0 {
            Btn::text_fg("<").build(ctx, "previous trips", None)
        } else {
            Btn::text_fg("<").inactive(ctx)
        },
        format!(
            "{}-{} of {}",
            if total_rows > 0 {
                prettyprint_usize(opts.skip + 1)
            } else {
                "0".to_string()
            },
            prettyprint_usize((opts.skip + 1 + ROWS).min(total_rows)),
            prettyprint_usize(total_rows)
        )
        .draw_text(ctx),
        if opts.skip + 1 + ROWS < total_rows {
            Btn::text_fg(">").build(ctx, "next trips", None)
        } else {
            Btn::text_fg(">").inactive(ctx)
        },
    ]));

    col.push(make_table(
        ctx,
        app,
        headers,
        rows,
        0.88 * ctx.canvas.window_width,
    ));
    col.push(
        Filler::new(ScreenDims::new(
            0.15 * ctx.canvas.window_width,
            0.15 * ctx.canvas.window_width,
        ))
        .named("preview")
        .centered_horiz(),
    );

    Composite::new(Widget::col(col))
        .exact_size_percent(90, 90)
        .build(ctx)
}

// TODO Figure out a nicer API to construct generic sortable tables.
pub fn make_table(
    ctx: &mut EventCtx,
    app: &App,
    headers: Vec<Widget>,
    rows: Vec<(String, Vec<GeomBatch>)>,
    total_width: f64,
) -> Widget {
    let total_width = total_width / ctx.get_scale_factor();
    let mut width_per_col: Vec<f64> = headers
        .iter()
        .map(|w| w.get_width_for_forcing() / ctx.get_scale_factor())
        .collect();
    for (_, row) in &rows {
        for (col, width) in row.iter().zip(width_per_col.iter_mut()) {
            *width = width.max(col.get_dims().width / ctx.get_scale_factor());
        }
    }
    let extra_margin = ((total_width - width_per_col.clone().into_iter().sum::<f64>())
        / (width_per_col.len() - 1) as f64)
        .max(0.0);

    let mut col = vec![Widget::custom_row(
        headers
            .into_iter()
            .enumerate()
            .map(|(idx, w)| {
                let margin = extra_margin + width_per_col[idx]
                    - (w.get_width_for_forcing() / ctx.get_scale_factor());
                if idx == width_per_col.len() - 1 {
                    w.margin_right((margin - extra_margin) as usize)
                } else {
                    w.margin_right(margin as usize)
                }
            })
            .collect(),
    )
    .bg(app.cs.section_bg)];

    for (label, row) in rows {
        let mut batch = GeomBatch::new();
        batch.autocrop_dims = false;
        let mut x1 = 0.0;
        for (col, width) in row.into_iter().zip(width_per_col.iter()) {
            batch.append(col.scale(1.0 / ctx.get_scale_factor()).translate(x1, 0.0));
            x1 += *width + extra_margin;
        }

        let rect = Polygon::rectangle(total_width, batch.get_dims().height);
        let mut hovered = GeomBatch::new();
        hovered.push(app.cs.hovering, rect.clone());
        hovered.append(batch.clone());

        col.push(
            Btn::custom(batch, hovered, rect)
                .tooltip(Text::new())
                .build(ctx, label, None),
        );
    }

    Widget::custom_col(col)
}

pub fn preview_trip(g: &mut GfxCtx, app: &App, composite: &Composite) {
    let inner_rect = composite.rect_of("preview").clone();
    let map_bounds = app.primary.map.get_bounds().clone();
    let zoom = 0.15 * g.canvas.window_width / map_bounds.width().max(map_bounds.height());
    g.fork(
        Pt2D::new(map_bounds.min_x, map_bounds.min_y),
        ScreenPt::new(inner_rect.x1, inner_rect.y1),
        zoom,
        None,
    );
    g.enable_clipping(inner_rect);

    g.redraw(&app.primary.draw_map.boundary_polygon);
    g.redraw(&app.primary.draw_map.draw_all_areas);
    g.redraw(
        &app.primary
            .draw_map
            .draw_all_unzoomed_roads_and_intersections,
    );

    if let Some(x) = composite.currently_hovering() {
        if let Ok(idx) = x.parse::<usize>() {
            let trip = TripID(idx);
            preview_route(g, app, trip).draw(g);
        }
    }

    g.disable_clipping();
    g.unfork();
}

fn preview_route(g: &mut GfxCtx, app: &App, trip: TripID) -> GeomBatch {
    let mut batch = GeomBatch::new();
    for p in app
        .primary
        .sim
        .get_analytics()
        .get_trip_phases(trip, &app.primary.map)
    {
        if let Some((dist, ref path)) = p.path {
            if let Some(trace) = path.trace(&app.primary.map, dist, None) {
                batch.push(
                    color_for_trip_phase(app, p.phase_type),
                    trace.make_polygons(Distance::meters(20.0)),
                );
            }
        }
    }

    let (_, start, end, _) = app.primary.sim.trip_info(trip);
    batch.append(
        GeomBatch::mapspace_svg(g.prerender, "system/assets/timeline/start_pos.svg")
            .scale(10.0)
            .color(RewriteColor::Change(Color::WHITE, Color::BLACK))
            .color(RewriteColor::Change(
                Color::hex("#5B5B5B"),
                Color::hex("#CC4121"),
            ))
            .centered_on(match start {
                TripEndpoint::Bldg(b) => app.primary.map.get_b(b).label_center,
                TripEndpoint::Border(i, _) => app.primary.map.get_i(i).polygon.center(),
            }),
    );
    batch.append(
        GeomBatch::mapspace_svg(g.prerender, "system/assets/timeline/goal_pos.svg")
            .scale(10.0)
            .color(RewriteColor::Change(Color::WHITE, Color::BLACK))
            .color(RewriteColor::Change(
                Color::hex("#5B5B5B"),
                Color::hex("#CC4121"),
            ))
            .centered_on(match end {
                TripEndpoint::Bldg(b) => app.primary.map.get_b(b).label_center,
                TripEndpoint::Border(i, _) => app.primary.map.get_i(i).polygon.center(),
            }),
    );

    batch
}
