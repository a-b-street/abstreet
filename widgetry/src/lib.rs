//! # Widgets
//!
//! If none of these do what you need, implementing a new [`WidgetImpl`] isn't tough.
//!
//! TODO inline pictures of some of these
//!
//! * [`Autocomplete`] - select predefined value by combining text entry with menus
//! * [`Button`] - clickable buttons with keybindings and tooltips
//! * [`Checkbox`] - toggle between two buttons
//! * [`CompareTimes`] - a scatter plot specialized for comparing times
//! * [`DrawWithTooltips`] - draw static geometry, with mouse tooltips in certain regions
//! * [`Dropdown`] - a button that expands into a menu
//! * [`FanChart`] - visualize a range of values over time
//! * [`Filler`] - just carve out space in the layout for something else
//! * [`JustDraw`] (argh private) - just draw text, `GeomBatch`es, SVGs
//! * [`LinePlot`] - visualize 2 variables with a line plot
//! * [`Menu`] - select something from a menu, with keybindings
//! * [`MultiButton`] - clickable regions in one batch of geometry
//! * [`PersistentSplit`] - a button with a dropdown to change its state
//! * [`ScatterPlot`] - visualize 2 variables with a scatter plot
//! * [`Slider`] - horizontal and vertical sliders
//! * [`Spinner`] - numeric input with up/down buttons
//! * [`table::Table`] - rows and columns, supporting filtering and pagination
//! * [`TexBox`] - single line text entry

//#![warn(missing_docs)]

#[macro_use]
extern crate log;

pub use crate::app_state::{DrawBaselayer, SharedAppState, SimpleState, State, Transition};
pub use crate::backend::Drawable;
pub use crate::canvas::{Canvas, HorizontalAlignment, VerticalAlignment};
pub use crate::color::{Color, Fill, LinearGradient, Texture};
pub use crate::drawing::{GfxCtx, Prerender};
pub use crate::event::{hotkeys, lctrl, Event, Key, MultiKey};
pub use crate::event_ctx::{EventCtx, UpdateType};
pub use crate::geom::{GeomBatch, RewriteColor};
pub use crate::input::UserInput;
pub use crate::runner::{run, Settings};
pub use crate::screen_geom::{ScreenDims, ScreenPt, ScreenRectangle};
pub use crate::style::Style;
pub use crate::text::{Line, Text, TextExt, TextSpan};
pub use crate::tools::warper::Warper;
pub use crate::tools::Cached;
pub use crate::widgets::autocomplete::Autocomplete;
pub(crate) use crate::widgets::button::Button;
pub use crate::widgets::button::{Btn, MultiButton};
pub use crate::widgets::checkbox::Checkbox;
pub use crate::widgets::compare_times::CompareTimes;
pub(crate) use crate::widgets::dropdown::Dropdown;
pub use crate::widgets::fan_chart::FanChart;
pub use crate::widgets::filler::Filler;
pub use crate::widgets::just_draw::DrawWithTooltips;
pub(crate) use crate::widgets::just_draw::{DeferDraw, JustDraw};
pub use crate::widgets::line_plot::{LinePlot, PlotOptions, Series};
pub use crate::widgets::menu::Menu;
pub use crate::widgets::persistent_split::PersistentSplit;
pub use crate::widgets::scatter_plot::ScatterPlot;
pub use crate::widgets::slider::Slider;
pub use crate::widgets::spinner::Spinner;
pub use crate::widgets::table;
pub(crate) use crate::widgets::text_box::TextBox;
pub use crate::widgets::{EdgeInsets, Outcome, Panel, Widget, WidgetImpl, WidgetOutput};

mod app_state;
mod assets;
#[cfg(any(feature = "native-backend", feature = "wasm-backend"))]
mod backend_glow;
#[cfg(feature = "native-backend")]
mod backend_glow_native;
#[cfg(feature = "wasm-backend")]
mod backend_glow_wasm;
mod canvas;
mod color;
mod drawing;
mod event;
mod event_ctx;
mod geom;
mod input;
mod runner;
mod screen_geom;
mod style;
mod svg;
mod text;
mod tools;
mod widgets;

mod backend {
    #[cfg(any(feature = "native-backend", feature = "wasm-backend"))]
    pub use crate::backend_glow::*;
}

pub struct Choice<T> {
    pub label: String,
    pub data: T,
    pub(crate) hotkey: Option<MultiKey>,
    pub(crate) active: bool,
    pub(crate) tooltip: Option<String>,
    pub(crate) fg: Option<Color>,
}

impl<T> Choice<T> {
    pub fn new<S: Into<String>>(label: S, data: T) -> Choice<T> {
        Choice {
            label: label.into(),
            data,
            hotkey: None,
            active: true,
            tooltip: None,
            fg: None,
        }
    }

    pub fn from(tuples: Vec<(String, T)>) -> Vec<Choice<T>> {
        tuples
            .into_iter()
            .map(|(label, data)| Choice::new(label, data))
            .collect()
    }

    pub fn key(mut self, key: Key) -> Choice<T> {
        assert_eq!(self.hotkey, None);
        self.hotkey = key.into();
        self
    }

    pub fn multikey(mut self, mk: Option<MultiKey>) -> Choice<T> {
        self.hotkey = mk;
        self
    }

    pub fn active(mut self, active: bool) -> Choice<T> {
        self.active = active;
        self
    }

    pub fn tooltip<I: Into<String>>(mut self, info: I) -> Choice<T> {
        self.tooltip = Some(info.into());
        self
    }

    pub fn fg(mut self, fg: Color) -> Choice<T> {
        self.fg = Some(fg);
        self
    }

    pub(crate) fn with_value<X>(&self, data: X) -> Choice<X> {
        Choice {
            label: self.label.clone(),
            data,
            hotkey: self.hotkey.clone(),
            active: self.active,
            tooltip: self.tooltip.clone(),
            fg: self.fg,
        }
    }
}

impl Choice<String> {
    pub fn string(label: &str) -> Choice<String> {
        Choice::new(label.to_string(), label.to_string())
    }

    pub fn strings<I: Into<String>>(list: Vec<I>) -> Vec<Choice<String>> {
        list.into_iter()
            .map(|x| {
                let x = x.into();
                Choice::new(x.clone(), x)
            })
            .collect()
    }
}
