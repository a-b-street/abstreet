mod assets;
mod canvas;
mod color;
mod drawing;
mod event;
mod event_ctx;
mod input;
pub mod layout;
mod managed;
mod runner;
mod screen_geom;
mod svg;
mod text;
mod widgets;

pub use crate::canvas::{Canvas, HorizontalAlignment, VerticalAlignment};
pub use crate::color::Color;
pub use crate::drawing::{DrawBoth, Drawable, GeomBatch, GfxCtx, Prerender, RewriteColor};
pub use crate::event::{hotkey, lctrl, Event, Key, MultiKey};
pub use crate::event_ctx::{EventCtx, TextureType};
pub use crate::input::UserInput;
pub use crate::managed::{Composite, ManagedWidget, Outcome};
pub use crate::runner::{run, EventLoopMode, Settings, GUI};
pub use crate::screen_geom::{ScreenDims, ScreenPt, ScreenRectangle};
pub use crate::text::{Line, Text, TextSpan, HOTKEY_COLOR};
pub use crate::widgets::{
    Autocomplete, Button, Choice, Filler, ItemSlider, JustDraw, ModalMenu, Plot, Series, Slider,
    SliderWithTextBox, Warper, WarpingItemSlider, Wizard, WrappedWizard,
};

pub enum InputResult<T: Clone> {
    Canceled,
    StillActive,
    Done(String, T),
}
