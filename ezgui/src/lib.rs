mod assets;
#[cfg(feature = "glium-backend")]
mod backend_glium;
#[cfg(feature = "glow-backend")]
mod backend_glow;
#[cfg(feature = "wasm-backend")]
mod backend_wasm;
mod canvas;
mod color;
mod drawing;
mod event;
mod event_ctx;
mod geom;
mod input;
mod managed;
mod runner;
mod screen_geom;
mod svg;
mod text;
mod tools;
mod widgets;

pub use crate::backend::Drawable;
pub use crate::canvas::{Canvas, HorizontalAlignment, VerticalAlignment};
pub use crate::color::Color;
pub use crate::drawing::{GfxCtx, Prerender};
pub use crate::event::{hotkey, hotkeys, lctrl, Event, Key, MultiKey};
pub use crate::event_ctx::EventCtx;
pub use crate::geom::{GeomBatch, RewriteColor};
pub use crate::input::UserInput;
pub use crate::managed::{Composite, Outcome, Widget};
pub use crate::runner::{run, EventLoopMode, Settings, GUI};
pub use crate::screen_geom::{ScreenDims, ScreenPt, ScreenRectangle};
pub use crate::text::{Line, Text, TextExt, TextSpan, HOTKEY_COLOR};
pub use crate::tools::warper::Warper;
pub use crate::widgets::autocomplete::Autocomplete;
pub use crate::widgets::button::Btn;
pub(crate) use crate::widgets::button::Button;
pub use crate::widgets::checkbox::Checkbox;
pub(crate) use crate::widgets::dropdown::Dropdown;
pub use crate::widgets::filler::Filler;
pub use crate::widgets::histogram::Histogram;
pub use crate::widgets::modal_menu::ModalMenu;
pub use crate::widgets::no_op::JustDraw;
pub use crate::widgets::plot::{Plot, PlotOptions, Series};
pub(crate) use crate::widgets::popup_menu::PopupMenu;
pub use crate::widgets::slider::{ItemSlider, Slider, WarpingItemSlider};
pub(crate) use crate::widgets::text_box::TextBox;
pub use crate::widgets::wizard::{Choice, Wizard, WrappedWizard};
pub(crate) use crate::widgets::WidgetImpl;

pub enum InputResult<T: Clone> {
    Canceled,
    StillActive,
    Done(String, T),
}

mod backend {
    #[cfg(feature = "glium-backend")]
    pub use crate::backend_glium::*;

    #[cfg(feature = "glow-backend")]
    pub use crate::backend_glow::*;

    #[cfg(feature = "wasm-backend")]
    pub use crate::backend_wasm::*;
}
