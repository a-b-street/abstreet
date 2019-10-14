use crate::{
    text, Canvas, Color, Drawable, EventCtx, GeomBatch, GfxCtx, Line, MultiText, ScreenDims,
    ScreenPt, ScreenRectangle, Text,
};
use geom::{Distance, Polygon, Pt2D};
use ordered_float::NotNan;

enum Item<T: Clone + Copy> {
    UpButton,
    DownButton,
    Actual(T),
}

// TODO Unify with Menu?
// TODO Handle window resizing generally
// TODO Hide scrolling buttons if not needed... or maybe that's an inconsistent UX
pub struct Scroller<T: Clone + Copy> {
    // TODO Maybe the height of each thing; insist that the width is the same for all?
    items: Vec<(Item<T>, ScreenDims)>,

    master_topleft: ScreenPt,
    hovering_on: Option<usize>,
    bg_color: Color,
    hovering_color: Color,
    current_selection_color: Color,

    // Does NOT include buttons!
    top_idx: usize,
    current_selection: usize,
}

impl<T: Clone + Copy> Scroller<T> {
    pub fn new(
        master_topleft: ScreenPt,
        actual_items: Vec<(T, ScreenDims)>,
        current_selection: usize,
        canvas: &Canvas,
    ) -> Scroller<T> {
        let max_width = actual_items
            .iter()
            .map(|(_, dims)| dims.width)
            .max_by_key(|w| NotNan::new(*w).unwrap())
            .unwrap();
        let mut items = vec![(
            Item::UpButton,
            ScreenDims::new(max_width, canvas.line_height),
        )];
        for (item, dims) in actual_items {
            items.push((Item::Actual(item), dims));
        }
        items.push((
            Item::DownButton,
            ScreenDims::new(max_width, canvas.line_height),
        ));

        let top_idx = current_selection;
        // TODO Try to start with current_selection centered, ideally. Or at least start a bit up
        // in this case. :\

        Scroller {
            items,
            master_topleft,
            hovering_on: None,
            // TODO ctx.cs
            bg_color: Color::BLACK.alpha(0.95),
            hovering_color: Color::RED.alpha(0.95),
            current_selection_color: Color::BLUE.alpha(0.95),
            top_idx,
            current_selection,
        }
    }

    // Includes buttons!
    fn get_visible_items(&self, canvas: &Canvas) -> Vec<(usize, ScreenRectangle)> {
        // Up button
        let mut visible = vec![(
            0,
            ScreenRectangle {
                x1: self.master_topleft.x,
                y1: self.master_topleft.y,
                x2: self.master_topleft.x + self.items[0].1.width,
                y2: self.master_topleft.y + self.items[0].1.height,
            },
        )];

        // Include the two buttons here
        let mut space_left = canvas.window_height - (2.0 * self.items[0].1.height);
        let mut y1 = visible[0].1.y2;

        for idx in 1 + self.top_idx..self.items.len() - 1 {
            if self.items[idx].1.height > space_left {
                break;
            }
            visible.push((
                idx,
                ScreenRectangle {
                    x1: self.master_topleft.x,
                    y1,
                    x2: self.master_topleft.x + self.items[idx].1.width,
                    y2: y1 + self.items[idx].1.height,
                },
            ));
            y1 += self.items[idx].1.height;
            space_left -= self.items[idx].1.height;
        }

        // Down button
        visible.push((
            self.items.len() - 1,
            ScreenRectangle {
                x1: self.master_topleft.x,
                y1,
                x2: self.master_topleft.x + self.items[0].1.width,
                y2: y1 + self.items[0].1.height,
            },
        ));

        visible
    }

    fn num_items_hidden_below(&self, canvas: &Canvas) -> usize {
        let visible = self.get_visible_items(canvas);
        // Ignore the down button
        let last_idx = visible[visible.len() - 2].0;
        self.items.len() - 2 - last_idx
    }

    // Returns the item selected, if it changes
    pub fn event(&mut self, ctx: &mut EventCtx) -> Option<T> {
        if ctx.redo_mouseover() {
            let cursor = ctx.canvas.get_cursor_in_screen_space();
            self.hovering_on = None;
            for (idx, rect) in self.get_visible_items(ctx.canvas) {
                if rect.contains(cursor) {
                    self.hovering_on = Some(idx);
                    break;
                }
            }
        }
        if let Some(idx) = self.hovering_on {
            if ctx.input.left_mouse_button_pressed() {
                match self.items[idx].0 {
                    Item::UpButton => {
                        if self.top_idx != 0 {
                            self.top_idx -= 1;
                        }
                    }
                    Item::DownButton => {
                        if self.num_items_hidden_below(ctx.canvas) != 0 {
                            self.top_idx += 1;
                        }
                    }
                    Item::Actual(item) => {
                        self.current_selection = idx - 1;
                        return Some(item);
                    }
                }
            }
        }

        None
    }

    // Returns the items to draw and the space they occupy.
    pub fn draw(&self, g: &mut GfxCtx) -> Vec<(T, ScreenRectangle)> {
        let visible = self.get_visible_items(g.canvas);
        // We know buttons have the max_width.
        let max_width = visible[0].1.width();
        let mut total_height = 0.0;
        for (_, rect) in &visible {
            total_height += rect.height();
        }

        g.fork_screenspace();
        g.draw_polygon(
            self.bg_color,
            &Polygon::rectangle_topleft(
                Pt2D::new(self.master_topleft.x, self.master_topleft.y),
                Distance::meters(max_width),
                Distance::meters(total_height),
            ),
        );
        g.canvas.mark_covered_area(ScreenRectangle::top_left(
            self.master_topleft,
            ScreenDims::new(max_width, total_height),
        ));

        let mut items = Vec::new();
        for (idx, rect) in visible {
            if Some(idx) == self.hovering_on || idx == self.current_selection + 1 {
                // Drawing text keeps reseting this. :(
                g.fork_screenspace();
                g.draw_polygon(
                    if Some(idx) == self.hovering_on {
                        self.hovering_color
                    } else {
                        self.current_selection_color
                    },
                    &Polygon::rectangle_topleft(
                        Pt2D::new(rect.x1, rect.y1),
                        Distance::meters(rect.width()),
                        Distance::meters(rect.height()),
                    ),
                );
            }
            match self.items[idx].0 {
                Item::UpButton => {
                    // TODO center the text inside the rectangle. and actually, g should have a
                    // method for that.
                    let mut txt = Text::with_bg_color(None);
                    if self.top_idx == 0 {
                        // TODO text::INACTIVE_CHOICE_COLOR
                        txt.add(Line("scroll up").fg(Color::grey(0.4)));
                    } else {
                        txt.add(Line(format!("scroll up ({} more items)", self.top_idx)));
                    }
                    g.draw_text_at_screenspace_topleft(&txt, ScreenPt::new(rect.x1, rect.y1));
                }
                Item::DownButton => {
                    let mut txt = Text::with_bg_color(None);
                    let num_items = self.num_items_hidden_below(g.canvas);
                    if num_items == 0 {
                        txt.add(Line("scroll down").fg(Color::grey(0.4)));
                    } else {
                        txt.add(Line(format!("scroll down ({} more items)", num_items)));
                    }
                    g.draw_text_at_screenspace_topleft(&txt, ScreenPt::new(rect.x1, rect.y1));
                }
                Item::Actual(item) => {
                    items.push((item, rect));
                }
            }
        }
        g.unfork();

        items
    }

    pub fn select_previous(&mut self) {
        assert!(self.current_selection != 0);
        self.current_selection -= 1;
        // TODO This and the case below aren't right; we might scroll far past the current
        // selection. Need similar logic for initializing Scroller and make sure the new
        // current_selection is "centered", but also retain consistency.
        if self.current_selection < self.top_idx {
            self.top_idx -= 1;
        }
    }

    pub fn select_next(&mut self, canvas: &Canvas) {
        assert!(self.current_selection != self.items.len() - 2);
        self.current_selection += 1;
        // Remember, the indices include buttons. :(
        if self
            .get_visible_items(canvas)
            .into_iter()
            .find(|(idx, _)| self.current_selection + 1 == *idx)
            .is_none()
        {
            self.top_idx += 1;
        }
    }

    pub fn current_idx(&self) -> usize {
        self.current_selection
    }

    pub fn num_items(&self) -> usize {
        self.items.len() - 2
    }
}

pub struct NewScroller {
    draw: Drawable,
    multi_txt: MultiText,
    total_dims: ScreenDims,
    zoom: f64,

    offset: f64,

    top_left: ScreenPt,
    dims: ScreenDims,
}

impl NewScroller {
    // geom and multi_txt should be in screen-space, with the top_left as (0.0, 0.0).
    pub fn new(geom: GeomBatch, multi_txt: MultiText, zoom: f64, ctx: &EventCtx) -> NewScroller {
        let mut total_dims = geom.get_dims();
        for (txt, top_left) in &multi_txt.list {
            let (mut w, mut h) = ctx.canvas.text_dims(txt);
            w += top_left.x;
            h += top_left.y;
            if w > total_dims.width {
                total_dims.width = w;
            }
            if h > total_dims.height {
                total_dims.height = h;
            }
        }

        NewScroller {
            draw: ctx.prerender.upload(geom),
            multi_txt,
            total_dims,
            zoom,

            offset: 0.0,

            // TODO The layouting is hardcoded
            top_left: ScreenPt::new(0.0, 0.0),
            dims: ScreenDims::new(100.0, 100.0),
        }
    }

    pub fn event(&mut self, ctx: &mut EventCtx) {
        let rect = ScreenRectangle::top_left(self.top_left, self.dims);
        if rect.contains(ctx.canvas.get_cursor_in_screen_space()) {
            if let Some(scroll) = ctx.input.get_mouse_scroll() {
                self.offset -= scroll;
                // TODO Clamp... or maybe last minute, based on dims, which'll get updated by
                // window resizing and such
            }
        }
    }

    pub fn draw(&self, g: &mut GfxCtx) {
        let rect = ScreenRectangle::top_left(self.top_left, self.dims);
        g.canvas.mark_covered_area(rect);

        g.fork_screenspace();
        g.draw_polygon(
            text::BG_COLOR,
            &Polygon::rectangle_topleft(
                Pt2D::new(0.0, 0.0),
                Distance::meters(self.dims.width),
                Distance::meters(self.dims.height),
            ),
        );
        g.unfork();

        g.fork(Pt2D::new(0.0, self.offset), self.top_left, self.zoom);
        g.redraw(&self.draw);
        g.unfork();

        for (txt, pt) in &self.multi_txt.list {
            g.draw_text_at_screenspace_topleft(
                txt,
                ScreenPt::new(pt.x, pt.y - self.offset * self.zoom),
            );
        }

        // TODO draw scrollbar
    }
}
