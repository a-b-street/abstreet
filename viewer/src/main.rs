mod model;

use crate::model::{World, ID};
use ezgui::{Canvas, Color, EventLoopMode, GfxCtx, Key, Prerender, Text, UserInput, GUI};
use std::{env, process};

struct UI {
    canvas: Canvas,
    world: World,
    state: State,
}

struct State {
    selected: Option<ID>,
}

impl UI {
    fn new(world: World, canvas: Canvas) -> UI {
        UI {
            canvas,
            world,
            state: State { selected: None },
        }
    }
}

impl GUI<Text> for UI {
    fn event(&mut self, input: &mut UserInput, _: &Prerender) -> (EventLoopMode, Text) {
        self.canvas.handle_event(input);

        if !self.canvas.is_dragging() && input.get_moved_mouse().is_some() {
            self.state.selected = self.world.mouseover_something(&self.canvas);
        }

        if input.unimportant_key_pressed(Key::Escape, "quit") {
            process::exit(0);
        }

        let mut osd = Text::new();
        input.populate_osd(&mut osd);
        (EventLoopMode::InputOnly, osd)
    }

    fn get_mut_canvas(&mut self) -> &mut Canvas {
        &mut self.canvas
    }

    fn draw(&self, g: &mut GfxCtx, osd: &Text) {
        g.clear(Color::WHITE);

        self.world.draw(g, &self.canvas);

        if let Some(id) = self.state.selected {
            self.world.draw_selected(g, &self.canvas, id);
        }

        self.canvas
            .draw_blocking_text(g, osd.clone(), ezgui::BOTTOM_LEFT);
    }
}

fn main() {
    let args: Vec<String> = env::args().collect();
    ezgui::run(
        "Generic viewer of things",
        1024.0,
        768.0,
        |canvas, prerender| UI::new(World::load_initial_map(&args[1], prerender), canvas),
    );
}
