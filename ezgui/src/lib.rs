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
mod input;
pub mod layout;
mod managed;
mod runner;
mod screen_geom;
mod svg;
mod text;
mod widgets;

pub use crate::backend::Drawable;
pub use crate::canvas::{Canvas, HorizontalAlignment, VerticalAlignment};
pub use crate::color::Color;
pub use crate::drawing::{GeomBatch, GfxCtx, Prerender, RewriteColor};
pub use crate::event::{hotkey, hotkeys, lctrl, Event, Key, MultiKey};
pub use crate::event_ctx::EventCtx;
pub use crate::input::UserInput;
pub use crate::managed::{Composite, ManagedWidget, Outcome};
pub use crate::runner::{run, EventLoopMode, Settings, GUI};
pub use crate::screen_geom::{ScreenDims, ScreenPt, ScreenRectangle};
pub use crate::text::{Line, Text, TextExt, TextSpan, HOTKEY_COLOR};
pub use crate::widgets::{
    Autocomplete, Btn, Button, Choice, Filler, Histogram, ItemSlider, JustDraw, ModalMenu, Plot,
    PlotOptions, Series, Slider, Warper, WarpingItemSlider, Wizard, WrappedWizard,
};

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
