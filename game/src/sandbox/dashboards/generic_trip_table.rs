use geom::{Distance, Pt2D};
use sim::{TripEndpoint, TripID};
use widgetry::{GeomBatch, GfxCtx, Panel, ScreenPt};

use crate::app::{App, Transition};
use crate::common::color_for_trip_phase;
use crate::info::{OpenTrip, Tab};
use crate::sandbox::SandboxMode;

pub(crate) fn open_trip_transition(app: &App, idx: usize) -> Transition {
    let trip = TripID(idx);
    let person = app.primary.sim.trip_to_person(trip).unwrap();

    Transition::Multi(vec![
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
    ])
}

pub(crate) fn preview_trip(g: &mut GfxCtx, app: &App, panel: &Panel, mut batch: GeomBatch) {
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
            preview_route(g, app, trip, &mut batch);
        }
    }
    batch.draw(g);

    g.disable_clipping();
    g.unfork();
}

fn preview_route(g: &mut GfxCtx, app: &App, id: TripID, batch: &mut GeomBatch) {
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
    batch.append(map_gui::tools::start_marker(
        g,
        match trip.start {
            TripEndpoint::Bldg(b) => app.primary.map.get_b(b).label_center,
            TripEndpoint::Border(i) => app.primary.map.get_i(i).polygon.center(),
            TripEndpoint::SuddenlyAppear(pos) => pos.pt(&app.primary.map),
        },
        5.0,
    ));
    batch.append(map_gui::tools::goal_marker(
        g,
        match trip.end {
            TripEndpoint::Bldg(b) => app.primary.map.get_b(b).label_center,
            TripEndpoint::Border(i) => app.primary.map.get_i(i).polygon.center(),
            TripEndpoint::SuddenlyAppear(pos) => pos.pt(&app.primary.map),
        },
        5.0,
    ));
}
