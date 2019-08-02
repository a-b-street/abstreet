use crate::render::{DrawCtx, DrawTurn};
use ezgui::{
    Canvas, Color, EventCtx, GeomBatch, GfxCtx, ModalMenu, ScreenDims, ScreenPt, ScreenRectangle,
    Text,
};
use geom::{Circle, Distance, Duration, PolyLine, Polygon, Pt2D};
use map_model::{Cycle, IntersectionID, Map, TurnPriority, TurnType, LANE_THICKNESS};
use ordered_float::NotNan;

// Only draws a box when time_left is present
pub fn draw_signal_cycle(
    cycle: &Cycle,
    time_left: Option<Duration>,
    batch: &mut GeomBatch,
    ctx: &DrawCtx,
) {
    if false {
        draw_signal_cycle_with_icons(cycle, batch, ctx);
        return;
    }

    let priority_color = ctx
        .cs
        .get_def("turns protected by traffic signal right now", Color::GREEN);
    let yield_color = ctx.cs.get_def(
        "turns allowed with yielding by traffic signal right now",
        Color::rgba(255, 105, 180, 0.8),
    );

    for (id, crosswalk) in &ctx.draw_map.get_i(cycle.parent).crosswalks {
        if cycle.get_priority(*id) == TurnPriority::Priority {
            batch.append(crosswalk);
        }
    }

    for t in &cycle.priority_turns {
        let turn = ctx.map.get_t(*t);
        if !turn.between_sidewalks() {
            DrawTurn::full_geom(turn, batch, priority_color);
        }
    }
    for t in &cycle.yield_turns {
        let turn = ctx.map.get_t(*t);
        // Lane-changing as yield is implied and very messy to show.
        if !turn.between_sidewalks()
            && turn.turn_type != TurnType::LaneChangeLeft
            && turn.turn_type != TurnType::LaneChangeRight
        {
            DrawTurn::outline_geom(turn, batch, yield_color);
        }
    }

    if time_left.is_none() {
        return;
    }

    let radius = Distance::meters(0.5);
    let box_width = 2.5 * radius;
    let box_height = 6.5 * radius;
    let center = ctx.map.get_i(cycle.parent).polygon.center();
    let top_left = center.offset(-box_width / 2.0, -box_height / 2.0);
    let percent = time_left.unwrap() / cycle.duration;
    // TODO Tune colors.
    batch.push(
        ctx.cs.get_def("traffic signal box", Color::grey(0.2)),
        Polygon::rectangle_topleft(top_left, box_width, box_height),
    );
    batch.push(
        Color::RED,
        Circle::new(center.offset(Distance::ZERO, -2.0 * radius), radius).to_polygon(),
    );
    batch.push(Color::grey(0.4), Circle::new(center, radius).to_polygon());
    batch.push(
        Color::YELLOW,
        Circle::new(center, radius).to_partial_polygon(percent),
    );
    batch.push(
        Color::GREEN,
        Circle::new(center.offset(Distance::ZERO, 2.0 * radius), radius).to_polygon(),
    );
}

fn draw_signal_cycle_with_icons(cycle: &Cycle, batch: &mut GeomBatch, ctx: &DrawCtx) {
    for l in &ctx.map.get_i(cycle.parent).incoming_lanes {
        let lane = ctx.map.get_l(*l);
        // TODO Show a hand or a walking sign for crosswalks
        if lane.is_parking() || lane.is_sidewalk() {
            continue;
        }
        let lane_line = lane.last_line();

        let mut _right_ok = true; // if not, no right turn on red
        let mut straight_green = true; // if not, the main light is red
                                       // TODO Multiple lefts?
        let mut left_priority: Option<TurnPriority> = None;
        for (turn, _) in ctx.map.get_next_turns_and_lanes(lane.id, cycle.parent) {
            match turn.turn_type {
                TurnType::SharedSidewalkCorner | TurnType::Crosswalk => unreachable!(),
                TurnType::Right => {
                    if cycle.get_priority(turn.id) == TurnPriority::Banned {
                        _right_ok = false;
                    }
                }
                TurnType::Straight | TurnType::LaneChangeLeft | TurnType::LaneChangeRight => {
                    // TODO Can we ever have Straight as Yield?
                    if cycle.get_priority(turn.id) == TurnPriority::Banned {
                        straight_green = false;
                    }
                }
                TurnType::Left => {
                    left_priority = Some(cycle.get_priority(turn.id));
                }
            };
        }

        let radius = LANE_THICKNESS / 2.0;

        // TODO Ignore right_ok...
        {
            let center1 = lane_line.unbounded_dist_along(lane_line.length() + radius);
            let color = if straight_green {
                ctx.cs.get_def("traffic light go", Color::GREEN)
            } else {
                ctx.cs.get_def("traffic light stop", Color::RED)
            };
            batch.push(color, Circle::new(center1, radius).to_polygon());
        }

        if let Some(pri) = left_priority {
            let center2 = lane_line.unbounded_dist_along(lane_line.length() + (radius * 3.0));
            let color = match pri {
                TurnPriority::Priority => ctx.cs.get("traffic light go"),
                // TODO flashing green
                TurnPriority::Yield => ctx.cs.get_def("traffic light permitted", Color::YELLOW),
                TurnPriority::Banned => ctx.cs.get("traffic light stop"),
                TurnPriority::Stop => unreachable!(),
            };
            batch.push(
                ctx.cs.get_def("traffic light box", Color::BLACK),
                Circle::new(center2, radius).to_polygon(),
            );
            batch.push(
                color,
                PolyLine::new(vec![
                    center2.project_away(radius, lane_line.angle().rotate_degs(90.0)),
                    center2.project_away(radius, lane_line.angle().rotate_degs(-90.0)),
                ])
                .make_arrow(Distance::meters(0.1))
                .unwrap(),
            );
        }
    }
}

const PADDING: f64 = 5.0;
// TODO Reduce this after testing out scrolling
const ZOOM: f64 = 15.0;

pub struct TrafficSignalDiagram {
    pub i: IntersectionID,
    labels: Vec<Text>,
    top_left: Pt2D,
    intersection_width: f64, // TODO needed?
    // The usizes are cycle indices
    scroller: Scroller<usize>,
}

impl TrafficSignalDiagram {
    pub fn new(
        i: IntersectionID,
        current_cycle: usize,
        map: &Map,
        ctx: &EventCtx,
    ) -> TrafficSignalDiagram {
        let (top_left, intersection_width, intersection_height) = {
            let b = map.get_i(i).polygon.get_bounds();
            (
                Pt2D::new(b.min_x, b.min_y),
                b.max_x - b.min_x,
                // Vertically pad
                b.max_y - b.min_y,
            )
        };
        let cycles = &map.get_traffic_signal(i).cycles;

        // Precalculate maximum text width.
        let mut labels = Vec::new();
        for (idx, cycle) in cycles.iter().enumerate() {
            labels.push(Text::from_line(format!(
                "Cycle {}: {}",
                idx + 1,
                cycle.duration
            )));
        }
        let label_length = labels
            .iter()
            .map(|l| ctx.canvas.text_dims(l).0)
            .max_by_key(|w| NotNan::new(*w).unwrap())
            .unwrap();
        let item_dims = ScreenDims::new(
            (intersection_width * ZOOM) + label_length + 10.0,
            (PADDING + intersection_height) * ZOOM,
        );

        let scroller = Scroller::new(
            ScreenPt::new(0.0, 0.0),
            std::iter::repeat(item_dims)
                .take(cycles.len())
                .enumerate()
                .collect(),
            current_cycle,
            ctx,
        );

        TrafficSignalDiagram {
            i,
            labels,
            top_left,
            intersection_width,
            scroller,
        }
    }

    pub fn event(&mut self, ctx: &mut EventCtx, menu: &mut ModalMenu) {
        self.scroller.event(ctx);

        if self.scroller.current_selection != 0 && menu.action("select previous cycle") {
            self.scroller.select_previous();
            return;
        }
        if self.scroller.current_selection != self.scroller.items.len() - 3
            && menu.action("select next cycle")
        {
            self.scroller.select_next(ctx.canvas);
            return;
        }
    }

    //*self = TrafficSignalDiagram::new(self.i, self.scroller.current_selection, &ui.primary.map, ctx);

    pub fn current_cycle(&self) -> usize {
        self.scroller.current_selection
    }

    pub fn draw(&self, g: &mut GfxCtx, ctx: &DrawCtx) {
        let cycles = &ctx.map.get_traffic_signal(self.i).cycles;

        for (idx, rect) in self.scroller.draw(g) {
            g.fork(self.top_left, ScreenPt::new(rect.x1, rect.y1), ZOOM);
            let mut batch = GeomBatch::new();
            draw_signal_cycle(&cycles[idx], None, &mut batch, ctx);
            batch.draw(g);

            g.draw_text_at_screenspace_topleft(
                &self.labels[idx],
                // TODO The x here is weird...
                ScreenPt::new(10.0 + (self.intersection_width * ZOOM), rect.y1),
            );
        }

        g.unfork();
    }
}

enum Item<T: Clone + Copy> {
    UpButton,
    DownButton,
    ActualItem(T),
}

struct Scroller<T: Clone + Copy> {
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
    fn new(
        master_topleft: ScreenPt,
        actual_items: Vec<(T, ScreenDims)>,
        current_selection: usize,
        ctx: &EventCtx,
    ) -> Scroller<T> {
        let max_width = actual_items
            .iter()
            .map(|(_, dims)| dims.width)
            .max_by_key(|w| NotNan::new(*w).unwrap())
            .unwrap();
        let (_, button_height) = ctx.canvas.text_dims(&Text::from_line("dummy".to_string()));
        let mut items = vec![(Item::UpButton, ScreenDims::new(max_width, button_height))];
        for (item, dims) in actual_items {
            items.push((Item::ActualItem(item), dims));
        }
        items.push((Item::DownButton, ScreenDims::new(max_width, button_height)));

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

    // Returns the item selected, if it changes
    fn event(&mut self, ctx: &mut EventCtx) -> Option<T> {
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
                        let visible = self.get_visible_items(ctx.canvas);
                        // Ignore the down button
                        let last_idx = visible[visible.len() - 2].0;
                        if last_idx != self.items.len() - 2 {
                            self.top_idx += 1;
                        }
                    }
                    Item::ActualItem(item) => {
                        self.current_selection = idx - 1;
                        return Some(item);
                    }
                }
            }
        }

        None
    }

    // Returns the items to draw and the space they occupy.
    fn draw(&self, g: &mut GfxCtx) -> Vec<(T, ScreenRectangle)> {
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
                    txt.add_line("scroll up".to_string());
                    g.draw_text_at_screenspace_topleft(&txt, ScreenPt::new(rect.x1, rect.y1));
                }
                Item::DownButton => {
                    let mut txt = Text::with_bg_color(None);
                    txt.add_line("scroll down".to_string());
                    g.draw_text_at_screenspace_topleft(&txt, ScreenPt::new(rect.x1, rect.y1));
                }
                Item::ActualItem(item) => {
                    items.push((item, rect));
                }
            }
        }
        g.unfork();

        items
    }

    fn select_previous(&mut self) {
        assert!(self.current_selection != 0);
        self.current_selection -= 1;
        // TODO This and the case below aren't right; we might scroll far past the current
        // selection. Need similar logic for initializing Scroller and make sure the new
        // current_selection is "centered", but also retain consistency.
        if self.current_selection < self.top_idx {
            self.top_idx -= 1;
        }
    }

    fn select_next(&mut self, canvas: &Canvas) {
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
}
