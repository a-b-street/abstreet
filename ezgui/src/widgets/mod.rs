pub mod autocomplete;
pub mod button;
pub mod checkbox;
pub mod containers;
pub mod dropdown;
pub mod filler;
pub mod histogram;
pub mod no_op;
pub mod plot;
pub mod popup_menu;
pub mod slider;
pub mod text_box;
pub mod wizard;

use crate::{EventCtx, GfxCtx, Outcome, ScreenDims, ScreenPt};

/// Create a new widget by implementing this trait. You can instantiate your widget by calling
/// `Widget::new(Box::new(instance of your new widget))`, which gives you the usual style options.
pub trait WidgetImpl: downcast_rs::Downcast {
    /// What width and height does the widget occupy? If this changes, be sure to set
    /// `redo_layout` to true in `event`.
    fn get_dims(&self) -> ScreenDims;
    /// Your widget's top left corner should be here. Handle mouse events and draw appropriately.
    fn set_pos(&mut self, top_left: ScreenPt);
    /// Your chance to react to an event. If this event should trigger layouting to be recalculated
    /// (because this widget changes dimensions), set `redo_layout` to true. Most widgets should
    /// return `None` instead of an `Outcome`.
    fn event(&mut self, ctx: &mut EventCtx, redo_layout: &mut bool) -> Option<Outcome>;
    /// Draw the widget. Be sure to draw relative to the top-left specified by `set_pos`.
    fn draw(&self, g: &mut GfxCtx);
}

downcast_rs::impl_downcast!(WidgetImpl);
