use std::cell::RefCell;
use std::rc::Rc;

use crate::{EventCtx, GfxCtx, ScreenDims, ScreenPt, Widget, WidgetImpl, WidgetOutput};

/// An invisible widget that stores some arbitrary data on the Panel. Users of the panel can read
/// and write the value. This is one method for "returning" data when a State completes.
pub struct Stash<T> {
    value: Rc<RefCell<T>>,
}

impl<T: 'static> Stash<T> {
    pub fn new_widget(name: &str, value: T) -> Widget {
        Widget::new(Box::new(Stash {
            value: Rc::new(RefCell::new(value)),
        }))
        .named(name)
    }

    pub(crate) fn get_value(&self) -> Rc<RefCell<T>> {
        self.value.clone()
    }
}

impl<T: 'static> WidgetImpl for Stash<T> {
    fn get_dims(&self) -> ScreenDims {
        ScreenDims::square(0.0)
    }

    fn set_pos(&mut self, _: ScreenPt) {}

    fn event(&mut self, _: &mut EventCtx, _: &mut WidgetOutput) {}
    fn draw(&self, _: &mut GfxCtx) {}
}
