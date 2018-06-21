// Copyright 2018 Google LLC, licensed under http://www.apache.org/licenses/LICENSE-2.0

use ezgui::canvas::GfxCtx;
use ezgui::input::UserInput;
use graphics;
use piston::input::{Key, MouseCursorEvent};
use piston::window::Size;

// TODO assumes minimum screen size
const WIDTH: u32 = 255;
const HEIGHT: u32 = 255;
const TILE_DIMS: u32 = 2;

// TODO should probably be a base widget in ezgui or something
pub struct ColorPicker {
    // This is an alternative to ToggleableLayer when there's a whole stateful plugin associated
    active: bool,
}

impl ColorPicker {
    pub fn new() -> ColorPicker {
        ColorPicker { active: false }
    }

    pub fn handle_event(self, input: &mut UserInput, window_size: &Size) -> ColorPicker {
        if !self.active {
            if input.unimportant_key_pressed(Key::D8, "Press 8 to configure colors") {
                return ColorPicker { active: true };
            }
            return ColorPicker::new();
        }

        if input.key_pressed(Key::D8, "Press 8 to stop configuring colors") {
            return ColorPicker::new();
        }

        if let Some(pos) = input.use_event_directly().mouse_cursor_args() {
            // TODO argh too much casting
            let (start_x, start_y) = self.get_screen_offset(window_size);
            let x = (pos[0] - (start_x as f64)) / (TILE_DIMS as f64) / 255.0;
            let y = (pos[1] - (start_y as f64)) / (TILE_DIMS as f64) / 255.0;
            if x >= 0.0 && x <= 1.0 && y >= 0.0 && y <= 1.0 {
                println!("current color is {:?}", get_color(x as f32, y as f32));
            }
        }

        return ColorPicker { active: true };
    }

    pub fn draw(&self, g: &mut GfxCtx) {
        if !self.active {
            return;
        }

        let (start_x, start_y) = self.get_screen_offset(&g.window_size);

        for x in 0..WIDTH {
            for y in 0..HEIGHT {
                let color = get_color((x as f32) / 255.0, (y as f32) / 255.0);
                let pixel = graphics::Rectangle::new(color);
                pixel.draw(
                    [
                        (x * TILE_DIMS + start_x) as f64,
                        (y * TILE_DIMS + start_y) as f64,
                        TILE_DIMS as f64,
                        TILE_DIMS as f64,
                    ],
                    &g.orig_ctx.draw_state,
                    g.orig_ctx.transform,
                    g.gfx,
                );
            }
        }
    }

    fn get_screen_offset(&self, window_size: &Size) -> (u32, u32) {
        let total_width = TILE_DIMS * WIDTH;
        let total_height = TILE_DIMS * HEIGHT;
        let start_x = (window_size.width - total_width) / 2;
        let start_y = (window_size.height - total_height) / 2;
        (start_x, start_y)
    }
}

fn get_color(x: f32, y: f32) -> graphics::types::Color {
    assert!(x >= 0.0 && x <= 1.0);
    assert!(y >= 0.0 && y <= 1.0);
    [x, y, (x + y) / 2.0, 1.0]
}
