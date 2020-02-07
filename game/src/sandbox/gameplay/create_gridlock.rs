use crate::game::Transition;
use crate::helpers::cmp_count_fewer;
use crate::managed::{WrappedComposite, WrappedOutcome};
use crate::render::InnerAgentColorScheme;
use crate::sandbox::gameplay::{challenge_controller, manage_acs, GameplayMode, GameplayState};
use crate::ui::UI;
use abstutil::prettyprint_usize;
use ezgui::{hotkey, layout, EventCtx, GfxCtx, Key, Line, ModalMenu, Text};
use geom::Time;
use sim::TripMode;

pub struct CreateGridlock {
    time: Time,
    menu: ModalMenu,
    top_center: WrappedComposite,
}

impl CreateGridlock {
    pub fn new(ctx: &mut EventCtx) -> Box<dyn GameplayState> {
        Box::new(CreateGridlock {
            time: Time::START_OF_DAY,
            menu: ModalMenu::new("", vec![(hotkey(Key::E), "show agent delay")], ctx)
                .set_standalone_layout(layout::ContainerOrientation::TopLeftButDownABit(150.0)),
            top_center: challenge_controller(
                ctx,
                GameplayMode::CreateGridlock,
                "Gridlock Challenge",
                Vec::new(),
            ),
        })
    }
}

impl GameplayState for CreateGridlock {
    fn event(&mut self, ctx: &mut EventCtx, ui: &mut UI) -> Option<Transition> {
        match self.top_center.event(ctx, ui) {
            Some(WrappedOutcome::Transition(t)) => {
                return Some(t);
            }
            Some(WrappedOutcome::Clicked(_)) => unreachable!(),
            None => {}
        }

        self.menu.event(ctx);
        manage_acs(
            &mut self.menu,
            ui,
            "show agent delay",
            "hide agent delay",
            InnerAgentColorScheme::Delay,
        );

        if self.time != ui.primary.sim.time() {
            self.time = ui.primary.sim.time();
            self.menu.set_info(gridlock_panel(ui));
        }

        None
    }

    fn draw(&self, g: &mut GfxCtx, _: &UI) {
        self.top_center.draw(g);
        self.menu.draw(g);
    }
}

fn gridlock_panel(ui: &UI) -> Text {
    let (now_all, _, now_per_mode) = ui
        .primary
        .sim
        .get_analytics()
        .all_finished_trips(ui.primary.sim.time());
    let (baseline_all, _, baseline_per_mode) =
        ui.prebaked().all_finished_trips(ui.primary.sim.time());

    let mut txt = Text::new();
    txt.add_appended(vec![
        Line(format!(
            "{} total finished trips (",
            prettyprint_usize(now_all.count())
        )),
        cmp_count_fewer(now_all.count(), baseline_all.count()),
        Line(")"),
    ]);

    for mode in TripMode::all() {
        let a = now_per_mode[&mode].count();
        let b = baseline_per_mode[&mode].count();
        txt.add_appended(vec![
            Line(format!("  {}: {} (", mode, prettyprint_usize(a))),
            cmp_count_fewer(a, b),
            Line(")"),
        ]);
    }

    txt
}
