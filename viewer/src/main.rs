mod model;

use crate::model::{World, ID};
use abstutil::{find_next_file, find_prev_file};
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

        if let Some(prev) = find_prev_file(&self.world.name) {
            if ctx.input.key_pressed(Key::Comma, "load previous map") {
                self.world = World::load_initial_map(&prev, ctx.prerender);
                self.state.selected = None;
            }
        }
        if let Some(next) = find_next_file(&self.world.name) {
            if ctx.input.key_pressed(Key::Dot, "load next map") {
                self.world = World::load_initial_map(&next, ctx.prerender);
                self.state.selected = None;
            }
        }

        let mut osd = Text::new();
        ctx.input.populate_osd(&mut osd);
        (EventLoopMode::InputOnly, osd)
    }

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
