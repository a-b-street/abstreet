//! # Widgets
//!
//! If none of these do what you need, implementing a new [`WidgetImpl`] isn't tough.
//!
//! TODO inline pictures of some of these
//!
//! * [`AreaSlider`] - slider with an associated area graph
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
//! * [`PersistentSplit`] - a button with a dropdown to change its state
//! * [`ScatterPlot`] - visualize 2 variables with a scatter plot
//! * [`Slider`] - horizontal and vertical sliders
//! * [`Spinner`] - numeric input with up/down buttons
//! * [`TexBox`] - single line text entry

//#![warn(missing_docs)]

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
mod style;
mod svg;
mod text;
mod tools;
mod widgets;

mod backend {
    #[cfg(feature = "glium-backend")]
    pub use crate::backend_glium::*;

    #[cfg(feature = "glow-backend")]
    pub use crate::backend_glow::*;

    #[cfg(feature = "wasm-backend")]
    pub use crate::backend_wasm::*;
}

pub use crate::backend::Drawable;
pub use crate::canvas::{Canvas, HorizontalAlignment, VerticalAlignment};
pub use crate::color::{Color, FancyColor, LinearGradient};
pub use crate::drawing::{GfxCtx, Prerender};
pub use crate::event::{hotkey, hotkeys, lctrl, Event, Key, MultiKey};
pub use crate::event_ctx::{EventCtx, UpdateType};
pub use crate::geom::{GeomBatch, RewriteColor};
pub use crate::input::UserInput;
pub use crate::managed::{Composite, Widget};
pub use crate::runner::{run, Settings, GUI};
pub use crate::screen_geom::{ScreenDims, ScreenPt, ScreenRectangle};
pub use crate::style::Style;
pub use crate::text::{Line, Text, TextExt, TextSpan};
pub use crate::tools::warper::Warper;
pub use crate::widgets::autocomplete::Autocomplete;
pub use crate::widgets::button::Btn;
pub(crate) use crate::widgets::button::Button;
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
pub use crate::widgets::slider::{AreaSlider, Slider};
pub use crate::widgets::spinner::Spinner;
pub(crate) use crate::widgets::text_box::TextBox;
pub use crate::widgets::{Outcome, WidgetImpl, WidgetOutput};

pub struct Choice<T> {
    pub label: String,
    pub data: T,
    pub(crate) hotkey: Option<MultiKey>,
    pub(crate) active: bool,
    pub(crate) tooltip: Option<String>,
}

impl<T> Choice<T> {
    pub fn new<S: Into<String>>(label: S, data: T) -> Choice<T> {
        Choice {
            label: label.into(),
            data,
            hotkey: None,
            active: true,
            tooltip: None,
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
        self.hotkey = hotkey(key);
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

    pub(crate) fn with_value<X>(&self, data: X) -> Choice<X> {
        Choice {
            label: self.label.clone(),
            data,
            hotkey: self.hotkey.clone(),
            active: self.active,
            tooltip: self.tooltip.clone(),
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
