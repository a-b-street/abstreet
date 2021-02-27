use geom::{Duration, Time};
use map_model::{BusRouteID, EditCmd};
use widgetry::{
    EventCtx, GfxCtx, HorizontalAlignment, Key, Line, Outcome, Panel, Spinner, State,
    StyledButtons, TextExt, VerticalAlignment, Widget,
};

use crate::app::App;
use crate::app::Transition;
use crate::edit::apply_map_edits;

pub struct RouteEditor {
    panel: Panel,
    route: BusRouteID,
}

impl RouteEditor {
    pub fn new(ctx: &mut EventCtx, app: &mut App, id: BusRouteID) -> Box<dyn State<App>> {
        app.primary.current_selection = None;

        let route = app.primary.map.get_br(id);
        Box::new(RouteEditor {
            panel: Panel::new(Widget::col(vec![
                Widget::row(vec![
                    Line("Route editor").small_heading().draw(ctx),
                    ctx.style().btn_close_widget(ctx),
                ]),
                Line(&route.full_name).draw(ctx),
                // TODO This UI needs design, just something to start plumbing the edits
                Widget::row(vec![
                    "Frequency in minutes".draw_text(ctx),
                    Spinner::widget(ctx, (1, 120), 60).named("freq_mins"),
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

        match self.panel.event(ctx) {
            Outcome::Clicked(x) => match x.as_ref() {
                "close" => {
                    return Transition::Pop;
                }
                "Apply" => {
                    let freq = Duration::minutes(self.panel.spinner("freq_mins") as usize);
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
            },
            _ => {}
        }

        Transition::Keep
    }

    fn draw(&self, g: &mut GfxCtx, _: &App) {
        self.panel.draw(g);
    }
}
