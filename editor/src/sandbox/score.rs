use crate::ui::UI;
use ezgui::{
    hotkey, EventCtx, GfxCtx, HorizontalAlignment, Key, ModalMenu, Text, VerticalAlignment,
};
use geom::DurationHistogram;
use itertools::Itertools;

pub struct Scoreboard {
    menu: ModalMenu,
    summary: Text,
}

impl Scoreboard {
    pub fn new(ctx: &mut EventCtx, ui: &UI) -> Scoreboard {
        let menu = ModalMenu::new("Scoreboard", vec![(hotkey(Key::Escape), "quit")], ctx);
        let t = ui.primary.sim.get_finished_trips();

        let mut summary = Text::new();
        summary.push(format!("Score at [red:{}]", ui.primary.sim.time()));
        summary.push(format!("[cyan:{}] unfinished trips", t.unfinished_trips));

        for (mode, trips) in &t
            .finished_trips
            .into_iter()
            .sorted_by_key(|(_, m, _)| *m)
            .group_by(|(_, m, _)| *m)
        {
            let mut distrib: DurationHistogram = std::default::Default::default();
            for (_, _, dt) in trips {
                distrib.add(dt);
            }
            summary.push(format!("[cyan:{:?}] trips: {}", mode, distrib.describe()));
        }

        Scoreboard { menu, summary }
    }

    // Returns true if done and we should go back to main sandbox mode.
    pub fn event(&mut self, ctx: &mut EventCtx) -> bool {
        self.menu.handle_event(ctx, None);
        if self.menu.action("quit") {
            return true;
        }
        false
    }

    pub fn draw(&self, g: &mut GfxCtx) {
        g.draw_blocking_text(
            &self.summary,
            (HorizontalAlignment::Center, VerticalAlignment::Center),
        );
        self.menu.draw(g);
    }
}
