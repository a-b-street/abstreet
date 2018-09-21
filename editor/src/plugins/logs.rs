use ezgui::{Canvas, GfxCtx, LogScroller, UserInput};
use objects::ROOT_MENU;
use piston::input::Key;
use plugins::Colorizer;

// TODO This is all total boilerplate!
pub enum DisplayLogs {
    Inactive,
    Active(LogScroller),
}

impl DisplayLogs {
    pub fn new() -> DisplayLogs {
        DisplayLogs::Inactive
    }

    pub fn event(&mut self, input: &mut UserInput) -> bool {
        let mut new_state: Option<DisplayLogs> = None;
        match self {
            DisplayLogs::Inactive => {
                if input.unimportant_key_pressed(
                    Key::Comma,
                    ROOT_MENU,
                    "show logs",
                ) {
                    let mut scroller = LogScroller::new_with_capacity(100);
                    for i in 0..150 {
                        scroller.add_line(&format!("Sup line {}", i));
                    }
                    new_state = Some(DisplayLogs::Active(scroller));
                }
            }
            DisplayLogs::Active(ref mut scroller) => {
                if scroller.event(input) {
                    new_state = Some(DisplayLogs::Inactive);
                }
            }
        }
        if let Some(s) = new_state {
            *self = s;
        }
        match self {
            DisplayLogs::Inactive => false,
            _ => true,
        }
    }

    pub fn draw(&self, g: &mut GfxCtx, canvas: &Canvas) {
        if let DisplayLogs::Active(scroller) = self {
            scroller.draw(g, canvas);
        }
    }
}

impl Colorizer for DisplayLogs {}
