mod canvas;
mod color;
mod drawing;
mod event;
mod event_ctx;
mod input;
mod log_scroller;
mod menu;
mod runner;
mod screen_geom;
mod screenshot;
mod scrolling_menu;
mod text;
mod text_box;
mod top_menu;
mod wizard;

pub use crate::canvas::{Canvas, HorizontalAlignment, VerticalAlignment, BOTTOM_LEFT, CENTERED};
pub use crate::color::Color;
pub use crate::drawing::GfxCtx;
pub use crate::event::{Event, Key};
pub use crate::event_ctx::{Drawable, EventCtx, Prerender};
pub use crate::input::{ModalMenu, UserInput};
pub use crate::log_scroller::LogScroller;
pub use crate::runner::{run, EventLoopMode, GUI};
pub use crate::screen_geom::ScreenPt;
pub use crate::scrolling_menu::ScrollingMenu;
pub use crate::text::Text;
pub use crate::text_box::TextBox;
pub use crate::top_menu::{Folder, TopMenu};
pub use crate::wizard::{Wizard, WrappedWizard};

pub struct ToggleableLayer {
    layer_name: String,
    // If None, never automatically enable at a certain zoom level.
    min_zoom: Option<f64>,

    enabled: bool,
}

impl ToggleableLayer {
    pub fn new(layer_name: &str, min_zoom: Option<f64>) -> ToggleableLayer {
        ToggleableLayer {
            min_zoom,
            layer_name: layer_name.to_string(),
            enabled: false,
        }
    }

    pub fn is_enabled(&self) -> bool {
        self.enabled
    }

    pub fn handle_zoom(&mut self, before_zoom: f64, after_zoom: f64) {
        if let Some(threshold) = self.min_zoom {
            let before_value = before_zoom >= threshold;
            let after_value = after_zoom >= threshold;
            if before_value != after_value {
                self.enabled = after_value;
            }
        }
    }

    // True if there was a change
    pub fn event(&mut self, input: &mut input::UserInput) -> bool {
        if input.action_chosen(&format!("show/hide {}", self.layer_name)) {
            self.enabled = !self.enabled;
            return true;
        }
        false
    }

    pub fn disable(&mut self) {
        self.enabled = false;
    }
}

pub enum InputResult<T: Clone> {
    Canceled,
    StillActive,
    Done(String, T),
}
