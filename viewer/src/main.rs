mod model;

use crate::model::{World, ID};
use ezgui::{Color, EventCtx, EventLoopMode, GfxCtx, Key, Text, GUI};
use std::{env, process};

struct UI {
    world: World,
    state: State,
}

struct State {
    selected: Option<ID>,
}

impl UI {
    fn new(world: World) -> UI {
        UI {
            world,
            state: State { selected: None },
        }
    }
}

impl GUI<Text> for UI {
    fn event(&mut self, ctx: EventCtx) -> (EventLoopMode, Text) {
        ctx.canvas.handle_event(ctx.input);

        if !ctx.canvas.is_dragging() && ctx.input.get_moved_mouse().is_some() {
            self.state.selected = self.world.mouseover_something(&ctx);
        }

        if ctx.input.unimportant_key_pressed(Key::Escape, "quit") {
            process::exit(0);
        }

        let mut osd = Text::new();
        ctx.input.populate_osd(&mut osd);
        (EventLoopMode::InputOnly, osd)
    }

    // TODO draw ctx should include the OSD!
    fn draw(&self, g: &mut GfxCtx, osd: &Text) {
        g.clear(Color::WHITE);

        self.world.draw(g);

        if let Some(id) = self.state.selected {
            self.world.draw_selected(g, id);
        }

        g.draw_blocking_text(osd.clone(), ezgui::BOTTOM_LEFT);
    }
}

fn main() {
    let args: Vec<String> = env::args().collect();
    ezgui::run("Generic viewer of things", 1024.0, 768.0, |_, prerender| {
        UI::new(World::load_initial_map(&args[1], prerender))
    });
}
