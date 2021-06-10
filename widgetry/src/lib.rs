//! # Widgets
//!
//! If none of these do what you need, implementing a new [`WidgetImpl`] isn't tough.
//!
//! TODO inline pictures of some of these
//!
//! * [`Autocomplete`] - select predefined value by combining text entry with menus
//! * [`Button`] - clickable buttons with keybindings and tooltips
//! * [`Toggle`] - checkboxes, switches, and other toggles
//! * [`CompareTimes`] - a scatter plot specialized for comparing times
//! * [`DragDrop`] - a reorderable row of draggable cards
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
//! * [`TextBox`] - single line text entry

//#![warn(missing_docs)]
#![allow(clippy::too_many_arguments, clippy::type_complexity)]
#![allow(clippy::new_without_default)]

#[macro_use]
extern crate anyhow;
#[macro_use]
extern crate log;

pub use crate::app_state::{DrawBaselayer, SharedAppState, SimpleState, State, Transition};
pub use crate::backend::Drawable;
pub use crate::canvas::{Canvas, CanvasSettings, HorizontalAlignment, VerticalAlignment};
pub use crate::color::{Color, Fill, LinearGradient, Texture};
pub use crate::drawing::{GfxCtx, Prerender};
pub use crate::event::{hotkeys, lctrl, Event, Key, MultiKey};
pub use crate::event_ctx::{EventCtx, UpdateType};
pub use crate::geom::geom_batch_stack::{
    Alignment as StackAlignment, Axis as StackAxis, GeomBatchStack,
};
pub use crate::geom::{GeomBatch, RewriteColor};
pub use crate::input::UserInput;
pub use crate::runner::{run, Settings};
pub use crate::screen_geom::{ScreenDims, ScreenPt, ScreenRectangle};
pub use crate::style::{ButtonStyle, OutlineStyle, Style};
pub use crate::text::{Font, Line, Text, TextExt, TextSpan};
pub use crate::tools::warper::Warper;
pub use crate::tools::Cached;
pub use crate::widgets::autocomplete::Autocomplete;
pub(crate) use crate::widgets::button::Button;
pub use crate::widgets::button::{ButtonBuilder, MultiButton};
pub use crate::widgets::compare_times::CompareTimes;
pub use crate::widgets::drag_drop::DragDrop;
pub(crate) use crate::widgets::dropdown::Dropdown;
pub use crate::widgets::fan_chart::FanChart;
pub use crate::widgets::filler::Filler;
pub use crate::widgets::image::{Image, ImageSource};
pub use crate::widgets::just_draw::DrawWithTooltips;
pub(crate) use crate::widgets::just_draw::{DeferDraw, JustDraw};
pub use crate::widgets::line_plot::LinePlot;
pub use crate::widgets::menu::Menu;
pub use crate::widgets::persistent_split::PersistentSplit;
pub use crate::widgets::plots::{PlotOptions, Series};
pub use crate::widgets::scatter_plot::ScatterPlot;
pub use crate::widgets::slider::Slider;
pub use crate::widgets::spinner::Spinner;
pub use crate::widgets::stash::Stash;
pub use crate::widgets::table;
pub use crate::widgets::tabs::TabController;
pub use crate::widgets::text_box::TextBox;
pub use crate::widgets::toggle::Toggle;
pub use crate::widgets::DEFAULT_CORNER_RADIUS;
pub use crate::widgets::{
    CornerRounding, EdgeInsets, Outcome, Panel, Widget, WidgetImpl, WidgetOutput,
};

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

/// Like [`std::include_bytes!`], but also returns its argument, the relative path to the bytes
///
/// returns a `(path, bytes): (&str, &[u8])` tuple
#[macro_export]
macro_rules! include_labeled_bytes {
    ($file:expr) => {
        ($file, include_bytes!($file))
    };
}

#[derive(Clone, Copy, Debug)]
pub enum ControlState {
    Default,
    Hovered,
    Disabled,
    // TODO: Pressing
}

/// Rules for how content should stretch to fill its bounds
#[derive(Clone, Copy, Debug)]
pub enum ContentMode {
    /// Stretches content to fit its bounds exactly, breaking aspect ratio as necessary.
    ScaleToFill,

    /// Maintaining aspect ratio, content grows until it touches its bounds in one dimension.
    /// This is the default ContentMode.
    ///
    /// If the aspect ratio of the bounds do not exactly match the aspect ratio of the content,
    /// then there will be some empty space within the bounds to center the content.
    ScaleAspectFit,

    /// Maintaining aspect ratio, content grows until both bounds are met.
    ///
    /// If the aspect ratio of the bounds do not exactly match the aspect ratio of the content,
    /// the content will overflow one dimension of its bounds.
    ScaleAspectFill,
}

impl Default for ContentMode {
    fn default() -> Self {
        ContentMode::ScaleAspectFit
    }
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

    pub fn multikey(mut self, mk: MultiKey) -> Choice<T> {
        self.hotkey = Some(mk);
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
