mod canvas;
mod color;
mod drawing;
mod event;
mod event_ctx;
mod input;
mod runner;
mod screen_geom;
mod text;
mod widgets;

pub use crate::canvas::{Canvas, HorizontalAlignment, VerticalAlignment, BOTTOM_LEFT, CENTERED};
pub use crate::color::Color;
pub use crate::drawing::GfxCtx;
pub use crate::event::{Event, Key};
pub use crate::event_ctx::{Drawable, EventCtx, Prerender};
pub use crate::input::UserInput;
pub use crate::runner::{run, EventLoopMode, GUI};
pub use crate::screen_geom::ScreenPt;
pub use crate::text::Text;
pub use crate::widgets::{
    Autocomplete, LogScroller, ModalMenu, ScrollingMenu, TextBox, Wizard, WrappedWizard,
};

pub enum InputResult<T: Clone> {
    Canceled,
    StillActive,
    Done(String, T),
}
