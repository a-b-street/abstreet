use crate::app::App;
use crate::edit::apply_map_edits;
use crate::game::{State, Transition};
use ezgui::{
    hotkey, Btn, Composite, EventCtx, GfxCtx, HorizontalAlignment, Key, Line, Outcome, Spinner,
    TextExt, VerticalAlignment, Widget,
};
use geom::{Duration, Time};
use map_model::{BusRouteID, EditCmd};

pub struct RouteEditor {
    composite: Composite,
    route: BusRouteID,
}

impl RouteEditor {
    pub fn new(ctx: &mut EventCtx, app: &mut App, id: BusRouteID) -> Box<dyn State> {
        app.primary.current_selection = None;

        let route = app.primary.map.get_br(id);
        Box::new(RouteEditor {
            composite: Composite::new(Widget::col(vec![
                Widget::row(vec![
                    Line("Route editor").small_heading().draw(ctx),
                    Btn::plaintext("X")
                        .build(ctx, "close", hotkey(Key::Escape))
                        .align_right(),
                ]),
                Line(&route.full_name).draw(ctx),
                // TODO This UI needs design, just something to start plumbing the edits
                Widget::row(vec![
                    "Frequency in minutes".draw_text(ctx),
                    Spinner::new(ctx, (1, 120), 60).named("freq_mins"),
                ]),
                Btn::text_bg2("Apply").build_def(ctx, hotkey(Key::Enter)),
            ]))
            .aligned(HorizontalAlignment::Center, VerticalAlignment::Top)
            .build(ctx),
            route: id,
        })
    }
}

impl State for RouteEditor {
    fn event(&mut self, ctx: &mut EventCtx, app: &mut App) -> Transition {
        ctx.canvas_movement();

        match self.composite.event(ctx) {
            Outcome::Clicked(x) => match x.as_ref() {
                "close" => {
                    return Transition::Pop;
                }
                "Apply" => {
                    let freq = Duration::minutes(self.composite.spinner("freq_mins") as usize);
                    let mut now = Time::START_OF_DAY;
                    let mut hourly_times = Vec::new();
                    while now <= Time::START_OF_DAY + Duration::hours(24) {
                        hourly_times.push(now);
                        now += freq;
                    }

                    let mut edits = app.primary.map.get_edits().clone();
                    edits.commands.push(EditCmd::ChangeRouteSchedule {
                        id: self.route,
                        old: app.primary.map.get_br(self.route).spawn_times.clone(),
                        new: hourly_times,
                    });
                    apply_map_edits(ctx, app, edits);

                    // TODO Hacks because we don't have an EditMode underneath us yet
                    app.primary.dirty_from_edits = true;
                    ctx.loading_screen("apply edits", |_, mut timer| {
                        app.primary
                            .map
                            .recalculate_pathfinding_after_edits(&mut timer);
                    });
                    // TODO Ah and actually we need to reset the sim and everything.

                    return Transition::Pop;
                }
                _ => unreachable!(),
            },
            _ => {}
        }

        Transition::Keep
    }

    fn draw(&self, g: &mut GfxCtx, _: &App) {
        self.composite.draw(g);
    }
}
