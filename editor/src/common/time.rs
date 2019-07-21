use crate::game::{State, Transition};
use crate::ui::UI;
use ezgui::{EventCtx, GfxCtx, ModalMenu, Wizard};
use geom::Duration;

pub fn time_controls(ctx: &mut EventCtx, ui: &mut UI, menu: &mut ModalMenu) -> Option<Transition> {
    if menu.action("step forwards 0.1s") {
        ui.primary.sim.step(&ui.primary.map, Duration::seconds(0.1));
        if let Some(ref mut s) = ui.secondary {
            s.sim.step(&s.map, Duration::seconds(0.1));
        }
        ui.recalculate_current_selection(ctx);
    } else if menu.action("step forwards 10 mins") {
        ctx.loading_screen("step forwards 10 minutes", |_, mut timer| {
            ui.primary
                .sim
                .timed_step(&ui.primary.map, Duration::minutes(10), &mut timer);
            if let Some(ref mut s) = ui.secondary {
                s.sim.timed_step(&s.map, Duration::minutes(10), &mut timer);
            }
        });
        ui.recalculate_current_selection(ctx);
    } else if menu.action("jump to specific time") {
        return Some(Transition::Push(Box::new(JumpingToTime {
            wizard: Wizard::new(),
        })));
    }
    None
}

struct JumpingToTime {
    wizard: Wizard,
}

impl State for JumpingToTime {
    fn event(&mut self, ctx: &mut EventCtx, ui: &mut UI) -> Transition {
        let mut wiz = self.wizard.wrap(ctx);

        if let Some(t) = wiz.input_time_slider(
            "Jump to what time?",
            ui.primary.sim.time(),
            Duration::END_OF_DAY,
        ) {
            let dt = t - ui.primary.sim.time();
            ctx.loading_screen(&format!("step forwards {}", dt), |_, mut timer| {
                ui.primary.sim.timed_step(&ui.primary.map, dt, &mut timer);
                if let Some(ref mut s) = ui.secondary {
                    s.sim.timed_step(&s.map, dt, &mut timer);
                }
            });
            return Transition::Pop;
        } else if self.wizard.aborted() {
            return Transition::Pop;
        }
        Transition::Keep
    }

    fn draw(&self, g: &mut GfxCtx, _: &UI) {
        self.wizard.draw(g);
    }
}
