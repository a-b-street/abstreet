//! # Widgets
//!
//! If none of these do what you need, implementing a new [`WidgetImpl`] isn't tough.
//!
//! TODO inline pictures of some of these
//!
//! * [`Autocomplete`] - select predefined value by combining text entry with menus
//! * [`Button`] - clickable buttons with keybindings and tooltips
//! * [`Checkbox`] - toggle between two buttons
//! * [`Dropdown`] - a button that expands into a menu
//! * [`Filler`] - just carve out space in the layout for something else
//! * [`Histogram`] - visualize a distribution
//! * [`JustDraw`] (argh private) - just draw text, `GeomBatch`es, SVGs
//! * [`Menu`] - select something from a menu, with keybindings
//! * [`PersistentSplit`] - a button with a dropdown to change its state
//! * [`Plot`] - visualize 2 variables with a line plot
//! * [`Slider`] - horizontal and vertical sliders
//! * [`Spinner`] - numeric input with up/down buttons
//! * [`TexBox`] - single line text entry

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
pub use crate::color::{Color, FancyColor, LinearGradient};
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
pub use crate::tools::wizard::{Choice, Wizard, WrappedWizard};
pub use crate::widgets::autocomplete::Autocomplete;
pub use crate::widgets::button::Btn;
pub(crate) use crate::widgets::button::Button;
pub use crate::widgets::checkbox::Checkbox;
pub(crate) use crate::widgets::dropdown::Dropdown;
pub use crate::widgets::filler::Filler;
pub use crate::widgets::histogram::Histogram;
pub(crate) use crate::widgets::just_draw::JustDraw;
pub(crate) use crate::widgets::menu::Menu;
pub use crate::widgets::persistent_split::PersistentSplit;
pub use crate::widgets::plot::{Plot, PlotOptions, Series};
pub use crate::widgets::slider::Slider;
pub use crate::widgets::spinner::Spinner;
pub(crate) use crate::widgets::text_box::TextBox;
pub use crate::widgets::WidgetImpl;

pub(crate) enum InputResult<T: Clone> {
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
