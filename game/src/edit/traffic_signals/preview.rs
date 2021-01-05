use std::collections::BTreeSet;

use abstutil::Timer;
use geom::Duration;
use map_gui::tools::ChooseSomething;
use map_model::IntersectionID;
use widgetry::{
    Btn, Choice, EventCtx, GfxCtx, HorizontalAlignment, Key, Outcome, Panel, State, TextExt,
    UpdateType, VerticalAlignment, Widget,
};

use crate::app::{App, Transition};
use crate::sandbox::{spawn_agents_around, SpeedControls, TimePanel};

// TODO Show diagram, auto-sync the stage.
// TODO Auto quit after things are gone?
struct PreviewTrafficSignal {
    panel: Panel,
    speed: SpeedControls,
    time_panel: TimePanel,
}

impl PreviewTrafficSignal {
    fn new(ctx: &mut EventCtx, app: &App) -> Box<dyn State<App>> {
        Box::new(PreviewTrafficSignal {
            panel: Panel::new(Widget::col(vec![
                "Previewing traffic signal".draw_text(ctx),
                Btn::text_fg("back to editing").build_def(ctx, Key::Escape),
            ]))
            .aligned(HorizontalAlignment::Center, VerticalAlignment::Top)
            .build(ctx),
            speed: SpeedControls::new(ctx, app),
            time_panel: TimePanel::new(ctx, app),
        })
    }
}

impl State<App> for PreviewTrafficSignal {
    fn event(&mut self, ctx: &mut EventCtx, app: &mut App) -> Transition {
        ctx.canvas_movement();

        match self.panel.event(ctx) {
            Outcome::Clicked(x) => match x.as_ref() {
                "back to editing" => {
                    app.primary.clear_sim();
                    return Transition::Pop;
                }
                _ => unreachable!(),
            },
            _ => {}
        }

        self.time_panel.event(ctx, app);
        // TODO Ideally here reset to midnight would jump back to when the preview started?
        if let Some(t) = self.speed.event(ctx, app, None) {
            return t;
        }
        if self.speed.is_paused() {
            Transition::Keep
        } else {
            ctx.request_update(UpdateType::Game);
            Transition::Keep
        }
    }

    fn draw(&self, g: &mut GfxCtx, _: &App) {
        self.panel.draw(g);
        self.speed.draw(g);
        self.time_panel.draw(g);
    }
}

// TODO I guess it's valid to preview without all turns possible. Some agents are just sad.
pub fn make_previewer(
    ctx: &mut EventCtx,
    app: &App,
    members: BTreeSet<IntersectionID>,
    stage: usize,
) -> Box<dyn State<App>> {
    let random = "random agents around these intersections".to_string();
    let right_now = format!(
        "change the traffic signal live at {}",
        app.primary.suspended_sim.as_ref().unwrap().time()
    );

    ChooseSomething::new(
        ctx,
        "Preview the traffic signal with what kind of traffic?",
        Choice::strings(vec![random, right_now]),
        Box::new(move |x, ctx, app| {
            if x == "random agents around these intersections" {
                for (idx, i) in members.iter().enumerate() {
                    if idx == 0 {
                        // Start at the current stage
                        let signal = app.primary.map.get_traffic_signal(*i);
                        // TODO Use the offset correctly
                        // TODO If there are variable stages, this could land anywhere
                        let mut step = Duration::ZERO;
                        for idx in 0..stage {
                            step += signal.stages[idx].stage_type.simple_duration();
                        }
                        app.primary.sim.timed_step(
                            &app.primary.map,
                            step,
                            &mut app.primary.sim_cb,
                            &mut Timer::throwaway(),
                        );
                    }

                    spawn_agents_around(*i, app);
                }
            } else {
                app.primary.sim = app.primary.suspended_sim.as_ref().unwrap().clone();
                app.primary
                    .sim
                    .handle_live_edited_traffic_signals(&app.primary.map);
            }
            Transition::Replace(PreviewTrafficSignal::new(ctx, app))
        }),
    )
}
