use crate::game::{State, Transition};
use crate::ui::UI;
use ezgui::{
    hotkey, Canvas, Color, EventCtx, GfxCtx, InputResult, Key, ModalMenu, ScreenPt, ScrollingMenu,
};
use geom::{Distance, Polygon};

// TODO assumes minimum screen size
const WIDTH: u32 = 255;
const HEIGHT: u32 = 255;
const TILE_DIMS: f64 = 2.0;

pub struct ColorChooser {
    menu: ScrollingMenu<()>,
}

impl ColorChooser {
    pub fn new(ui: &UI) -> ColorChooser {
        ColorChooser {
            menu: ScrollingMenu::new("Pick a color to change", ui.cs.color_names()),
        }
    }
}

impl State for ColorChooser {
    fn event(&mut self, ctx: &mut EventCtx, ui: &mut UI) -> Transition {
        match self.menu.event(&mut ctx.input) {
            InputResult::Canceled => Transition::Pop,
            InputResult::StillActive => Transition::Keep,
            InputResult::Done(name, _) => Transition::Replace(Box::new(ColorChanger {
                name: name.clone(),
                original: ui.cs.get_modified(&name),
                menu: ModalMenu::new(
                    &format!("Color Picker for {}", name),
                    vec![vec![
                        (hotkey(Key::Backspace), "revert"),
                        (hotkey(Key::Escape), "finalize"),
                    ]],
                    ctx,
                ),
            })),
        }
    }

    fn draw(&self, g: &mut GfxCtx, _: &UI) {
        self.menu.draw(g);
    }
}

struct ColorChanger {
    name: String,
    original: Option<Color>,
    menu: ModalMenu,
}

impl State for ColorChanger {
    fn event(&mut self, ctx: &mut EventCtx, ui: &mut UI) -> Transition {
        self.menu.handle_event(ctx, None);
        if self.menu.action("revert") {
            ui.cs.reset_modified(&self.name, self.original);
            return Transition::Pop;
        } else if self.menu.action("finalize") {
            println!("Setting color for {}", self.name);
            return Transition::Pop;
        }

        if let Some(pt) = ctx.input.get_moved_mouse() {
            // TODO argh too much casting
            let (start_x, start_y) = get_screen_offset(&ctx.canvas);
            let x = (pt.x - start_x) / TILE_DIMS / 255.0;
            let y = (pt.y - start_y) / TILE_DIMS / 255.0;
            if x >= 0.0 && x <= 1.0 && y >= 0.0 && y <= 1.0 {
                ui.cs
                    .override_color(&self.name, get_color(x as f32, y as f32));
            }
        }
        Transition::Keep
    }

    fn draw(&self, g: &mut GfxCtx, _: &UI) {
        let (start_x, start_y) = get_screen_offset(g.canvas);

        for x in 0..WIDTH {
            for y in 0..HEIGHT {
                let color = get_color((x as f32) / 255.0, (y as f32) / 255.0);
                let corner = g.screen_to_map(ScreenPt::new(
                    f64::from(x) * TILE_DIMS + start_x,
                    f64::from(y) * TILE_DIMS + start_y,
                ));
                g.draw_polygon(
                    color,
                    &Polygon::rectangle_topleft(
                        corner,
                        Distance::meters(TILE_DIMS),
                        Distance::meters(TILE_DIMS),
                    ),
                );
            }
        }
        self.menu.draw(g);
    }
}

fn get_screen_offset(canvas: &Canvas) -> (f64, f64) {
    let total_width = TILE_DIMS * f64::from(WIDTH);
    let total_height = TILE_DIMS * f64::from(HEIGHT);
    let start_x = (canvas.window_width - total_width) / 2.0;
    let start_y = (canvas.window_height - total_height) / 2.0;
    (start_x, start_y)
}

fn get_color(x: f32, y: f32) -> Color {
    assert!(x >= 0.0 && x <= 1.0);
    assert!(y >= 0.0 && y <= 1.0);
    Color::rgb_f(x, y, (x + y) / 2.0)
}
