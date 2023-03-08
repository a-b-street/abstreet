use std::cell::RefCell;
use std::collections::HashSet;
use std::rc::Rc;

use taffy::geometry::Size;
use taffy::layout::AvailableSpace;
use taffy::node::{Node, Taffy};
use taffy::style::{Dimension, Style};

use geom::Polygon;

use crate::widgets::slider;
use crate::widgets::spinner::SpinnerValue;
use crate::widgets::Container;
use crate::{
    Autocomplete, Button, Color, Dropdown, EventCtx, GfxCtx, HorizontalAlignment, Menu, Outcome,
    PersistentSplit, ScreenDims, ScreenPt, ScreenRectangle, Slider, Spinner, Stash, TextBox,
    Toggle, VerticalAlignment, Widget, WidgetImpl, WidgetOutput,
};

pub struct Panel {
    top_level: Widget,
    // (layout, root_dims)
    cached_flexbox: Option<(Taffy, Vec<Node>, ScreenDims)>,
    horiz: HorizontalAlignment,
    vert: VerticalAlignment,
    dims_x: PanelDims,
    dims_y: PanelDims,

    scrollable_x: bool,
    scrollable_y: bool,
    contents_dims: ScreenDims,
    container_dims: ScreenDims,
    clip_rect: Option<ScreenRectangle>,
}

impl Panel {
    pub fn new_builder(top_level: Widget) -> PanelBuilder {
        PanelBuilder {
            top_level,
            horiz: HorizontalAlignment::Center,
            vert: VerticalAlignment::Center,
            dims_x: PanelDims::MaxPercent(1.0),
            dims_y: PanelDims::MaxPercent(1.0),
            ignore_initial_events: false,
        }
    }

    /// Returns an empty panel. `event` and `draw` will have no effect.
    pub fn empty(ctx: &mut EventCtx) -> Panel {
        Panel::new_builder(Widget::col(vec![])).build_custom(ctx)
    }

    fn update_container_dims_for_canvas_dims(&mut self, canvas_dims: ScreenDims) {
        let width = match self.dims_x {
            PanelDims::MaxPercent(pct) => self.contents_dims.width.min(pct * canvas_dims.width),
            PanelDims::ExactPercent(pct) => pct * canvas_dims.width,
            PanelDims::ExactPixels(x) => x,
        };
        let height = match self.dims_y {
            PanelDims::MaxPercent(pct) => self.contents_dims.height.min(pct * canvas_dims.height),
            PanelDims::ExactPercent(pct) => pct * canvas_dims.height,
            PanelDims::ExactPixels(x) => x,
        };
        self.container_dims = ScreenDims::new(width, height);
    }

    fn recompute_scrollbar_layout(&mut self, ctx: &EventCtx) {
        let old_scrollable_x = self.scrollable_x;
        let old_scrollable_y = self.scrollable_y;
        let old_scroll_offset = self.scroll_offset();
        let mut was_dragging_x = false;
        let mut was_dragging_y = false;

        self.scrollable_x = self.contents_dims.width > self.container_dims.width;
        self.scrollable_y = self.contents_dims.height > self.container_dims.height;

        // Unwrap the main widget from any scrollable containers if necessary.
        if old_scrollable_y {
            let container = self.top_level.widget.downcast_mut::<Container>().unwrap();
            was_dragging_y = container.members[1]
                .widget
                .downcast_ref::<Slider>()
                .unwrap()
                .dragging;
            self.top_level = container.members.remove(0);
        }

        if old_scrollable_x {
            let container = self.top_level.widget.downcast_mut::<Container>().unwrap();
            was_dragging_x = container.members[1]
                .widget
                .downcast_ref::<Slider>()
                .unwrap()
                .dragging;
            self.top_level = container.members.remove(0);
        }

        let mut container_dims = self.container_dims;
        // TODO Handle making room for a horizontal scrollbar on the bottom. The equivalent change
        // to container_dims.height doesn't work as expected.
        if self.scrollable_y {
            container_dims.width += slider::SCROLLBAR_BG_WIDTH;
        }
        let top_left = ctx
            .canvas
            .align_window(container_dims, self.horiz, self.vert);

        // Wrap the main widget in scrollable containers if necessary.
        if self.scrollable_x {
            let mut slider = Slider::horizontal_scrollbar(
                ctx,
                self.container_dims.width,
                self.container_dims.width * (self.container_dims.width / self.contents_dims.width),
                0.0,
            )
            .named("horiz scrollbar")
            .abs(top_left.x, top_left.y + self.container_dims.height);
            // We constantly destroy and recreate the scrollbar slider while dragging it. Preserve
            // the dragging property, so we can keep dragging it.
            if was_dragging_x {
                slider.widget.downcast_mut::<Slider>().unwrap().dragging = true;
            }

            let old_top_level = std::mem::replace(&mut self.top_level, Widget::nothing());
            self.top_level = Widget::custom_col(vec![old_top_level, slider]);
        }

        if self.scrollable_y {
            let mut slider = Slider::vertical_scrollbar(
                ctx,
                self.container_dims.height,
                self.container_dims.height
                    * (self.container_dims.height / self.contents_dims.height),
                0.0,
            )
            .named("vert scrollbar")
            .abs(top_left.x + self.container_dims.width, top_left.y);
            if was_dragging_y {
                slider.widget.downcast_mut::<Slider>().unwrap().dragging = true;
            }

            let old_top_level = std::mem::replace(&mut self.top_level, Widget::nothing());
            self.top_level = Widget::custom_row(vec![old_top_level, slider]);
        }

        self.update_scroll_sliders(ctx, old_scroll_offset);

        self.clip_rect = if self.scrollable_x || self.scrollable_y {
            Some(ScreenRectangle::top_left(top_left, self.container_dims))
        } else {
            None
        };
    }

    // TODO: this method potentially gets called multiple times in a render pass as an
    // optimization, we could replace all the current call sites with a "dirty" flag, e.g.
    // `set_needs_layout()` and then call `layout_if_needed()` once at the last possible moment
    fn recompute_layout(&mut self, ctx: &EventCtx, recompute_bg: bool) {
        self.invalidate_flexbox();
        self.recompute_layout_if_needed(ctx, recompute_bg)
    }

    fn invalidate_flexbox(&mut self) {
        self.cached_flexbox = None;
    }

    fn compute_flexbox(&self) -> (Taffy, Vec<Node>, ScreenDims) {
        let mut taffy = Taffy::new();
        let root = taffy
            .new_with_children(
                Style {
                    ..Default::default()
                },
                &[],
            )
            .unwrap();

        let mut nodes = vec![];
        self.top_level.get_flexbox(root, &mut taffy, &mut nodes);
        nodes.reverse();

        // TODO Express more simply. Constraining this seems useless.
        let container_size = Size {
            width: AvailableSpace::MaxContent,
            height: AvailableSpace::MaxContent,
        };
        taffy.compute_layout(root, container_size).unwrap();

        // TODO I'm so confused why these 2 are acting differently. :(
        let effective_dims = if self.scrollable_x || self.scrollable_y {
            self.container_dims
        } else {
            let result = taffy.layout(root).unwrap();
            ScreenDims::new(result.size.width.into(), result.size.height.into())
        };

        (taffy, nodes, effective_dims)
    }

    fn recompute_layout_if_needed(&mut self, ctx: &EventCtx, recompute_bg: bool) {
        self.recompute_scrollbar_layout(ctx);
        let (taffy, nodes, effective_dims) = self
            .cached_flexbox
            .take()
            .unwrap_or_else(|| self.compute_flexbox());

        {
            let top_left = ctx
                .canvas
                .align_window(effective_dims, self.horiz, self.vert);
            let offset = self.scroll_offset();
            let mut nodes = nodes.clone();
            self.top_level.apply_flexbox(
                &taffy,
                &mut nodes,
                top_left.x,
                top_left.y,
                offset,
                ctx,
                recompute_bg,
                false,
            );
            assert!(nodes.is_empty());
        }
        self.cached_flexbox = Some((taffy, nodes, effective_dims));
    }

    fn scroll_offset(&self) -> (f64, f64) {
        let x = if self.scrollable_x {
            self.slider("horiz scrollbar").get_percent()
                * (self.contents_dims.width - self.container_dims.width).max(0.0)
        } else {
            0.0
        };
        let y = if self.scrollable_y {
            self.slider("vert scrollbar").get_percent()
                * (self.contents_dims.height - self.container_dims.height).max(0.0)
        } else {
            0.0
        };
        (x, y)
    }

    fn update_scroll_sliders(&mut self, ctx: &EventCtx, offset: (f64, f64)) -> bool {
        let mut changed = false;
        if self.scrollable_x {
            changed = true;
            let max = (self.contents_dims.width - self.container_dims.width).max(0.0);
            if max == 0.0 {
                self.slider_mut("horiz scrollbar").set_percent(ctx, 0.0);
            } else {
                self.slider_mut("horiz scrollbar")
                    .set_percent(ctx, offset.0.clamp(0.0, max) / max);
            }
        }
        if self.scrollable_y {
            changed = true;
            let max = (self.contents_dims.height - self.container_dims.height).max(0.0);
            if max == 0.0 {
                self.slider_mut("vert scrollbar").set_percent(ctx, 0.0);
            } else {
                self.slider_mut("vert scrollbar")
                    .set_percent(ctx, offset.1.clamp(0.0, max) / max);
            }
        }
        changed
    }

    fn set_scroll_offset(&mut self, ctx: &EventCtx, offset: (f64, f64)) {
        if self.update_scroll_sliders(ctx, offset) {
            self.recompute_layout_if_needed(ctx, false);
        }
    }

    pub fn event(&mut self, ctx: &mut EventCtx) -> Outcome {
        if (self.scrollable_x || self.scrollable_y)
            && ctx
                .canvas
                .get_cursor_in_screen_space()
                .map(|pt| self.top_level.rect.contains(pt))
                .unwrap_or(false)
        {
            if let Some((dx, dy)) = ctx.input.get_mouse_scroll() {
                let x_offset = if self.scrollable_x {
                    self.scroll_offset().0 - dx * (ctx.canvas.settings.gui_scroll_speed as f64)
                } else {
                    0.0
                };
                let y_offset = if self.scrollable_y {
                    self.scroll_offset().1 - dy * (ctx.canvas.settings.gui_scroll_speed as f64)
                } else {
                    0.0
                };
                self.set_scroll_offset(ctx, (x_offset, y_offset));
            }
        }

        if ctx.input.is_window_resized() {
            self.update_container_dims_for_canvas_dims(ctx.canvas.get_window_dims());
            self.recompute_layout(ctx, false);
        }

        let before = self.scroll_offset();
        let mut output = WidgetOutput::new();
        self.top_level.widget.event(ctx, &mut output);

        if output.redo_layout {
            self.recompute_layout(ctx, true);
        } else if self.scroll_offset() != before {
            self.recompute_layout_if_needed(ctx, true);
        }

        // Remember this for the next event
        if let Outcome::Focused(ref id) = output.outcome {
            assert!(ctx.next_focus_owned_by.is_none());
            ctx.next_focus_owned_by = Some(id.clone());
        }

        output.outcome
    }

    pub fn draw(&self, g: &mut GfxCtx) {
        if let Some(ref rect) = self.clip_rect {
            g.enable_clipping(rect.clone());
            g.canvas.mark_covered_area(rect.clone());
        } else {
            g.canvas.mark_covered_area(self.top_level.rect.clone());
        }

        // Debugging
        if false {
            g.fork_screenspace();
            g.draw_polygon(Color::RED.alpha(0.5), self.top_level.rect.to_polygon());

            let top_left = g
                .canvas
                .align_window(self.container_dims, self.horiz, self.vert);
            g.draw_polygon(
                Color::BLUE.alpha(0.5),
                Polygon::rectangle(self.container_dims.width, self.container_dims.height)
                    .translate(top_left.x, top_left.y),
            );
            g.unfork();
        }

        self.top_level.draw(g);
        if self.scrollable_x || self.scrollable_y {
            g.disable_clipping();

            // Draw the scrollbars after clipping is disabled, because they actually live just
            // outside the rectangle.
            if self.scrollable_x {
                self.slider("horiz scrollbar").draw(g);
            }
            if self.scrollable_y {
                self.slider("vert scrollbar").draw(g);
            }
        }
    }

    pub fn get_all_click_actions(&self) -> HashSet<String> {
        let mut actions = HashSet::new();
        self.top_level.get_all_click_actions(&mut actions);
        actions
    }

    pub fn restore(&mut self, ctx: &mut EventCtx, prev: &Panel) {
        self.set_scroll_offset(ctx, prev.scroll_offset());

        self.top_level.restore(ctx, prev);

        // Since we just moved things around, let all widgets respond to the mouse being somewhere
        ctx.no_op_event(true, |ctx| {
            assert!(matches!(self.event(ctx), Outcome::Nothing))
        });
    }

    pub fn restore_scroll(&mut self, ctx: &mut EventCtx, prev: &Panel) {
        self.set_scroll_offset(ctx, prev.scroll_offset());
    }

    pub fn scroll_to_member(&mut self, ctx: &EventCtx, name: String) {
        if let Some(w) = self.top_level.find(&name) {
            let y1 = w.rect.y1;
            self.set_scroll_offset(ctx, (0.0, y1));
        } else {
            panic!("Can't scroll_to_member of unknown {}", name);
        }
    }

    pub fn has_widget(&self, name: &str) -> bool {
        self.top_level.find(name).is_some()
    }

    pub fn slider(&self, name: &str) -> &Slider {
        self.find(name)
    }
    pub fn slider_mut(&mut self, name: &str) -> &mut Slider {
        self.find_mut(name)
    }

    pub fn take_menu_choice<T: 'static>(&mut self, name: &str) -> T {
        self.find_mut::<Menu<T>>(name).take_current_choice()
    }

    pub fn is_checked(&self, name: &str) -> bool {
        self.find::<Toggle>(name).enabled
    }
    pub fn maybe_is_checked(&self, name: &str) -> Option<bool> {
        if self.has_widget(name) {
            Some(self.find::<Toggle>(name).enabled)
        } else {
            None
        }
    }
    pub fn set_checked(&mut self, name: &str, on_off: bool) {
        self.find_mut::<Toggle>(name).enabled = on_off
    }

    pub fn text_box(&self, name: &str) -> String {
        self.find::<TextBox>(name).get_line()
    }

    pub fn spinner<T: 'static + SpinnerValue>(&self, name: &str) -> T {
        self.find::<Spinner<T>>(name).current
    }
    pub fn modify_spinner<T: 'static + SpinnerValue>(
        &mut self,
        ctx: &EventCtx,
        name: &str,
        delta: T,
    ) {
        self.find_mut::<Spinner<T>>(name).modify(ctx, delta)
    }

    pub fn dropdown_value<T: 'static + PartialEq + Clone, I: AsRef<str>>(&self, name: I) -> T {
        self.find::<Dropdown<T>>(name.as_ref()).current_value()
    }
    pub fn maybe_dropdown_value<T: 'static + PartialEq + Clone, I: AsRef<str>>(
        &self,
        name: I,
    ) -> Option<T> {
        let name = name.as_ref();
        if self.has_widget(name) {
            Some(self.find::<Dropdown<T>>(name).current_value())
        } else {
            None
        }
    }
    pub fn persistent_split_value<T: 'static + PartialEq + Clone>(&self, name: &str) -> T {
        self.find::<PersistentSplit<T>>(name).current_value()
    }

    /// Consumes the autocomplete widget. It's fine if the panel survives past this event; the
    /// autocomplete just needs to be interacted with again to produce more values.
    pub fn autocomplete_done<T: 'static + Clone>(&mut self, name: &str) -> Option<Vec<T>> {
        self.find_mut::<Autocomplete<T>>(name).take_final_value()
    }

    /// Grab a stashed value, with the ability to pass it around and modify it.
    pub fn stash<T: 'static>(&self, name: &str) -> Rc<RefCell<T>> {
        self.find::<Stash<T>>(name).get_value()
    }

    /// Grab a stashed value and clone it.
    pub fn clone_stashed<T: 'static + Clone>(&self, name: &str) -> T {
        self.find::<Stash<T>>(name).get_value().borrow().clone()
    }

    pub fn is_button_enabled(&self, name: &str) -> bool {
        self.find::<Button>(name).is_enabled()
    }

    pub fn maybe_find_widget(&self, name: &str) -> Option<&Widget> {
        self.top_level.find(name)
    }

    pub fn maybe_find<T: WidgetImpl>(&self, name: &str) -> Option<&T> {
        self.maybe_find_widget(name).map(|w| {
            if let Some(x) = w.widget.downcast_ref::<T>() {
                x
            } else {
                panic!("Found widget {}, but wrong type", name);
            }
        })
    }

    pub fn find<T: WidgetImpl>(&self, name: &str) -> &T {
        self.maybe_find(name)
            .unwrap_or_else(|| panic!("Can't find widget {}", name))
    }

    pub fn find_mut<T: WidgetImpl>(&mut self, name: &str) -> &mut T {
        if let Some(w) = self.top_level.find_mut(name) {
            if let Some(x) = w.widget.downcast_mut::<T>() {
                x
            } else {
                panic!("Found widget {}, but wrong type", name);
            }
        } else {
            panic!("Can't find widget {}", name);
        }
    }

    /// Swap the inner content of a `container` widget with `new_inner_content`.
    pub(crate) fn swap_inner_content(
        &mut self,
        ctx: &EventCtx,
        container_name: &str,
        new_inner_content: &mut Widget,
    ) {
        let old_container: &mut Container = self.find_mut(container_name);
        assert_eq!(
            old_container.members.len(),
            1,
            "method only intended to be used for containers created with `Widget::container`"
        );
        std::mem::swap(&mut old_container.members[0], new_inner_content);
        self.recompute_layout(ctx, true);
    }

    pub fn rect_of(&self, name: &str) -> &ScreenRectangle {
        &self.top_level.find(name).unwrap().rect
    }
    // TODO Deprecate
    pub fn center_of(&self, name: &str) -> ScreenPt {
        self.rect_of(name).center()
    }
    pub fn center_of_panel(&self) -> ScreenPt {
        self.top_level.rect.center()
    }
    pub fn panel_rect(&self) -> &ScreenRectangle {
        &self.top_level.rect
    }
    pub fn panel_dims(&self) -> ScreenDims {
        self.top_level.rect.dims()
    }

    pub fn align(&mut self, horiz: HorizontalAlignment, vert: VerticalAlignment) {
        self.horiz = horiz;
        self.vert = vert;
        // TODO Recompute layout and fire no_op_event?
    }

    /// All margins/padding/etc from the previous widget are retained. The ID is set on the new
    /// widget; no need to do that yourself.
    pub fn replace(&mut self, ctx: &mut EventCtx, id: &str, mut new: Widget) {
        if let Some(ref new_id) = new.id {
            assert_eq!(id, new_id);
        }
        new = new.named(id);
        let old = self
            .top_level
            .find_mut(id)
            .unwrap_or_else(|| panic!("Panel doesn't have {}", id));
        new.layout.style = old.layout.style;
        *old = new;
        self.recompute_layout(ctx, true);
        // TODO Since we just moved things around, let all widgets respond to the mouse being
        // somewhere? Maybe always do this in recompute_layout?
        //ctx.no_op_event(true, |ctx| assert!(matches!(self.event(ctx), Outcome::Nothing)));
    }

    /// Removes a widget from the panel. Does not recalculate layout!
    pub fn take(&mut self, id: &str) -> Widget {
        self.top_level.take(id).unwrap()
    }

    pub fn clicked_outside(&self, ctx: &mut EventCtx) -> bool {
        // TODO No great way to populate OSD from here with "click to cancel"
        !self.top_level.rect.contains(ctx.canvas.get_cursor()) && ctx.normal_left_click()
    }

    pub fn currently_hovering(&self) -> Option<&String> {
        self.top_level.currently_hovering()
    }
}

pub struct PanelBuilder {
    top_level: Widget,
    horiz: HorizontalAlignment,
    vert: VerticalAlignment,
    dims_x: PanelDims,
    dims_y: PanelDims,
    ignore_initial_events: bool,
}

#[derive(Clone, Copy)]
pub enum PanelDims {
    MaxPercent(f64),
    ExactPercent(f64),
    ExactPixels(f64),
}

impl PanelBuilder {
    pub fn build(mut self, ctx: &mut EventCtx) -> Panel {
        self.top_level = self.top_level.padding(16).bg(ctx.style.panel_bg);
        self.build_custom(ctx)
    }

    pub fn build_custom(self, ctx: &mut EventCtx) -> Panel {
        let ignore_initial_events = self.ignore_initial_events;
        let mut panel = Panel {
            top_level: self.top_level,

            horiz: self.horiz,
            vert: self.vert,
            dims_x: self.dims_x,
            dims_y: self.dims_y,

            scrollable_x: false,
            scrollable_y: false,
            contents_dims: ScreenDims::new(0.0, 0.0),
            container_dims: ScreenDims::new(0.0, 0.0),
            clip_rect: None,
            cached_flexbox: None,
        };
        match self.dims_x {
            PanelDims::MaxPercent(_) => {}
            PanelDims::ExactPercent(pct) => {
                // Don't set size, because then scrolling breaks -- the actual size has to be based
                // on the contents.
                panel.top_level.layout.style.min_size.width =
                    Dimension::Points((pct * ctx.canvas.window_width) as f32);
            }
            PanelDims::ExactPixels(x) => {
                panel.top_level.layout.style.min_size.width = Dimension::Points(x as f32);
            }
        }
        match self.dims_y {
            PanelDims::MaxPercent(_) => {}
            PanelDims::ExactPercent(pct) => {
                panel.top_level.layout.style.min_size.height =
                    Dimension::Points((pct * ctx.canvas.window_height) as f32);
            }
            PanelDims::ExactPixels(x) => {
                panel.top_level.layout.style.min_size.height = Dimension::Points(x as f32);
            }
        }

        // There is a dependency cycle in our layout logic. As a consequence:
        //   1. we have to call `recompute_layout` twice here
        //   2. panels don't responsively change `contents_dims`
        //
        // - `panel.top_level.rect`, used here to set content_dims, is set by `recompute_layout`.
        // - the output of `recompute_layout` depends on `container_dims`
        // - `container_dims`, in the case of `MaxPercent`, depend on `content_dims`
        //
        // TODO: to support Panel's that can resize their `contents_dims`, we'll need to detangle
        // this dependency. This might entail decomposing the flexbox calculation to layout first
        // the inner content, and then potentially a second pass to layout any x/y scrollbars.
        panel.recompute_layout(ctx, false);
        panel.contents_dims =
            ScreenDims::new(panel.top_level.rect.width(), panel.top_level.rect.height());
        panel.update_container_dims_for_canvas_dims(ctx.canvas.get_window_dims());
        panel.recompute_layout(ctx, false);

        // Just trigger error if a button is double-defined
        panel.get_all_click_actions();
        // Let all widgets initially respond to the mouse being somewhere
        ctx.no_op_event(true, |ctx| {
            if ignore_initial_events {
                panel.event(ctx);
            } else {
                let outcome = panel.event(ctx);
                if !matches!(outcome, Outcome::Nothing) {
                    panic!(
                        "Initial panel outcome is {}. Consider calling ignore_initial_events",
                        outcome.describe()
                    );
                }
            }
        });
        panel
    }

    pub fn aligned(mut self, horiz: HorizontalAlignment, vert: VerticalAlignment) -> PanelBuilder {
        self.horiz = horiz;
        self.vert = vert;
        self
    }

    pub fn aligned_pair(mut self, pair: (HorizontalAlignment, VerticalAlignment)) -> PanelBuilder {
        self.horiz = pair.0;
        self.vert = pair.1;
        self
    }

    pub fn dims_width(mut self, dims: PanelDims) -> PanelBuilder {
        self.dims_x = dims;
        self
    }

    pub fn dims_height(mut self, dims: PanelDims) -> PanelBuilder {
        self.dims_y = dims;
        self
    }

    // TODO Change all callers
    pub fn exact_size_percent(self, x: usize, y: usize) -> PanelBuilder {
        self.dims_width(PanelDims::ExactPercent((x as f64) / 100.0))
            .dims_height(PanelDims::ExactPercent((y as f64) / 100.0))
    }

    /// When a panel is built, a fake, "no-op" mouseover event is immediately fired, to let all
    /// widgets initially pick up the position of the mouse. Normally this event should only
    /// produce `Outcome::Nothing`, since other outcomes will be lost -- there's no way for the
    /// caller to see that first outcome.
    ///
    /// If a caller expects this first mouseover to possibly produce an outcome, they can call this
    /// and avoid the assertion.
    pub fn ignore_initial_events(mut self) -> PanelBuilder {
        self.ignore_initial_events = true;
        self
    }
}
