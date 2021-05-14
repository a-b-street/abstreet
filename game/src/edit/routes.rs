use geom::{Duration, Time};
use map_model::{BusRouteID, EditCmd};
use widgetry::{
    EventCtx, GfxCtx, HorizontalAlignment, Key, Line, Outcome, Panel, Spinner, State, TextExt,
    VerticalAlignment, Widget,
};

use crate::app::App;
use crate::app::Transition;
use crate::edit::apply_map_edits;

pub struct RouteEditor {
    panel: Panel,
    route: BusRouteID,
}

impl RouteEditor {
    pub fn new_state(ctx: &mut EventCtx, app: &mut App, id: BusRouteID) -> Box<dyn State<App>> {
        app.primary.current_selection = None;

        let route = app.primary.map.get_br(id);
        Box::new(RouteEditor {
            panel: Panel::new_builder(Widget::col(vec![
                Widget::row(vec![
                    Line("Route editor").small_heading().into_widget(ctx),
                    ctx.style().btn_close_widget(ctx),
                ]),
                Line(&route.full_name).into_widget(ctx),
                // TODO This UI needs design, just something to start plumbing the edits
                Widget::row(vec![
                    "Frequency".text_widget(ctx),
                    Spinner::widget(
                        ctx,
                        "freq_mins",
                        (Duration::minutes(1), Duration::hours(2)),
                        Duration::hours(1),
                        Duration::minutes(1),
                    ),
                ]),
                ctx.style()
                    .btn_solid_primary
                    .text("Apply")
                    .hotkey(Key::Enter)
                    .build_def(ctx),
            ]))
            .aligned(HorizontalAlignment::Center, VerticalAlignment::Top)
            .build(ctx),
            route: id,
        })
    }
}

impl State<App> for RouteEditor {
    fn event(&mut self, ctx: &mut EventCtx, app: &mut App) -> Transition {
        ctx.canvas_movement();

        if let Outcome::Clicked(x) = self.panel.event(ctx) {
            match x.as_ref() {
                "close" => {
                    return Transition::Pop;
                }
                "Apply" => {
                    let freq = self.panel.spinner("freq_mins");
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

                    return Transition::Pop;
                }
                _ => unreachable!(),
            }
        }

        Transition::Keep
    }

    fn draw(&self, g: &mut GfxCtx, _: &App) {
        self.panel.draw(g);
    }
}
