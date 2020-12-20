use geom::{Distance, Pt2D};
use sim::{TripEndpoint, TripID};
use widgetry::table::Table;
use widgetry::{
    Color, DrawBaselayer, EventCtx, GeomBatch, GfxCtx, Outcome, Panel, RewriteColor, ScreenPt,
    State,
};

use crate::app::{App, Transition};
use crate::common::color_for_trip_phase;
use crate::info::{OpenTrip, Tab};
use crate::sandbox::dashboards::trip_table;
use crate::sandbox::dashboards::DashTab;
use crate::sandbox::SandboxMode;

pub struct GenericTripTable<T, F, P: 'static + Fn(&mut EventCtx, &App, &Table<App, T, F>) -> Panel>
{
    table: Table<App, T, F>,
    panel: Panel,
    make_panel: P,
    tab: DashTab,
}

impl<T: 'static, F: 'static, P: 'static + Fn(&mut EventCtx, &App, &Table<App, T, F>) -> Panel>
    GenericTripTable<T, F, P>
{
    pub fn new(
        ctx: &mut EventCtx,
        app: &App,
        tab: DashTab,
        table: Table<App, T, F>,
        make_panel: P,
    ) -> Box<dyn State<App>> {
        let panel = (make_panel)(ctx, app, &table);
        Box::new(GenericTripTable {
            table,
            panel,
            make_panel,
            tab,
        })
    }

    fn recalc(&mut self, ctx: &mut EventCtx, app: &App) {
        let mut new = (self.make_panel)(ctx, app, &self.table);
        new.restore(ctx, &self.panel);
        self.panel = new;
    }
}

impl<T: 'static, F: 'static, P: 'static + Fn(&mut EventCtx, &App, &Table<App, T, F>) -> Panel>
    State<App> for GenericTripTable<T, F, P>
{
    fn event(&mut self, ctx: &mut EventCtx, app: &mut App) -> Transition {
        match self.panel.event(ctx) {
            Outcome::Clicked(x) => {
                if self.table.clicked(&x) {
                    self.recalc(ctx, app);
                } else if let Ok(idx) = x.parse::<usize>() {
                    let trip = TripID(idx);
                    let person = app.primary.sim.trip_to_person(trip).unwrap();
                    return Transition::Multi(vec![
                        Transition::Pop,
                        Transition::ModifyState(Box::new(move |state, ctx, app| {
                            let sandbox = state.downcast_mut::<SandboxMode>().unwrap();
                            let mut actions = sandbox.contextual_actions();
                            sandbox.controls.common.as_mut().unwrap().launch_info_panel(
                                ctx,
                                app,
                                Tab::PersonTrips(person, OpenTrip::single(trip)),
                                &mut actions,
                            );
                        })),
                    ]);
                } else if x == "close" {
                    return Transition::Pop;
                } else if x == "finished trips" {
                    return Transition::Replace(trip_table::FinishedTripTable::new(ctx, app));
                } else if x == "cancelled trips" {
                    return Transition::Replace(trip_table::CancelledTripTable::new(ctx, app));
                } else if x == "unfinished trips" {
                    return Transition::Replace(trip_table::UnfinishedTripTable::new(ctx, app));
                } else {
                    unreachable!()
                }
            }
            Outcome::Changed => {
                if let Some(t) = self.tab.transition(ctx, app, &self.panel) {
                    return t;
                }

                self.table.panel_changed(&self.panel);
                self.recalc(ctx, app);
            }
            _ => {}
        }

        Transition::Keep
    }

    fn draw_baselayer(&self) -> DrawBaselayer {
        DrawBaselayer::Custom
    }

    fn draw(&self, g: &mut GfxCtx, app: &App) {
        g.clear(app.cs.dialog_bg);
        self.panel.draw(g);
        preview_trip(g, app, &self.panel);
    }
}

fn preview_trip(g: &mut GfxCtx, app: &App, panel: &Panel) {
    let inner_rect = panel.rect_of("preview").clone();
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

    if let Some(x) = panel.currently_hovering() {
        if let Ok(idx) = x.parse::<usize>() {
            let trip = TripID(idx);
            preview_route(g, app, trip).draw(g);
        }
    }

    g.disable_clipping();
    g.unfork();
}

fn preview_route(g: &mut GfxCtx, app: &App, id: TripID) -> GeomBatch {
    let mut batch = GeomBatch::new();
    for p in app
        .primary
        .sim
        .get_analytics()
        .get_trip_phases(id, &app.primary.map)
    {
        if let Some(path) = &p.path {
            if let Some(trace) = path.trace(&app.primary.map) {
                batch.push(
                    color_for_trip_phase(app, p.phase_type),
                    trace.make_polygons(Distance::meters(20.0)),
                );
            }
        }
    }

    let trip = app.primary.sim.trip_info(id);
    batch.append(
        GeomBatch::load_svg(g, "system/assets/timeline/start_pos.svg")
            .scale(10.0)
            .color(RewriteColor::Change(Color::WHITE, Color::BLACK))
            .color(RewriteColor::Change(
                Color::hex("#5B5B5B"),
                Color::hex("#CC4121"),
            ))
            .centered_on(match trip.start {
                TripEndpoint::Bldg(b) => app.primary.map.get_b(b).label_center,
                TripEndpoint::Border(i) => app.primary.map.get_i(i).polygon.center(),
                TripEndpoint::SuddenlyAppear(pos) => pos.pt(&app.primary.map),
            }),
    );
    batch.append(
        GeomBatch::load_svg(g, "system/assets/timeline/goal_pos.svg")
            .scale(10.0)
            .color(RewriteColor::Change(Color::WHITE, Color::BLACK))
            .color(RewriteColor::Change(
                Color::hex("#5B5B5B"),
                Color::hex("#CC4121"),
            ))
            .centered_on(match trip.end {
                TripEndpoint::Bldg(b) => app.primary.map.get_b(b).label_center,
                TripEndpoint::Border(i) => app.primary.map.get_i(i).polygon.center(),
                TripEndpoint::SuddenlyAppear(pos) => pos.pt(&app.primary.map),
            }),
    );

    batch
}
