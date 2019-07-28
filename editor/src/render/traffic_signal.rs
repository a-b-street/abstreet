use crate::render::{DrawCtx, DrawTurn};
use crate::ui::UI;
use ezgui::{
    Color, EventCtx, GeomBatch, GfxCtx, ModalMenu, ScreenDims, ScreenPt, ScreenRectangle, Text,
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
    pub current_cycle: usize,
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
        let item_dims = ScreenDims {
            width: (intersection_width * ZOOM) + label_length + 10.0,
            height: (PADDING + intersection_height) * ZOOM,
        };

        let scroller = Scroller::new(
            ScreenPt::new(0.0, 0.0),
            cycles.iter().map(|cycle| (cycle.idx, item_dims)).collect(),
            ctx,
        );

        TrafficSignalDiagram {
            i,
            current_cycle,
            labels,
            top_left,
            intersection_width,
            scroller,
        }
    }

    pub fn event(&mut self, ctx: &mut EventCtx, ui: &UI, menu: &mut ModalMenu) {
        if let Some(idx) = self.scroller.event(ctx) {
            self.current_cycle = idx;
            self.reset(ui, ctx);
        }

        if self.current_cycle != 0 && menu.action("select previous cycle") {
            self.current_cycle -= 1;
            self.reset(ui, ctx);
            return;
        }
        if self.current_cycle != self.scroller.items.len() - 1 && menu.action("select next cycle") {
            self.current_cycle += 1;
            self.reset(ui, ctx);
            return;
        }
    }

    pub fn reset(&mut self, ui: &UI, ctx: &mut EventCtx) {
        *self = TrafficSignalDiagram::new(self.i, self.current_cycle, &ui.primary.map, ctx);
    }

    pub fn draw(&self, g: &mut GfxCtx, ctx: &DrawCtx) {
        let cycles = &ctx.map.get_traffic_signal(self.i).cycles;

        for (idx, screen_top_left, dims) in self.scroller.draw(g) {
            // TODO Maybe make Scroller do this after all.
            if idx == self.current_cycle {
                g.fork_screenspace();
                g.draw_polygon(
                    ctx.cs.get_def(
                        "current cycle in traffic signal panel",
                        Color::BLUE.alpha(0.95),
                    ),
                    &Polygon::rectangle_topleft(
                        Pt2D::new(screen_top_left.x, screen_top_left.y),
                        Distance::meters(dims.width),
                        Distance::meters(dims.height),
                    ),
                );
            }

            g.fork(self.top_left, screen_top_left, ZOOM);
            let mut batch = GeomBatch::new();
            draw_signal_cycle(&cycles[idx], None, &mut batch, ctx);
            batch.draw(g);

            g.draw_text_at_screenspace_topleft(
                &self.labels[idx],
                // TODO The x here is weird...
                ScreenPt::new(10.0 + (self.intersection_width * ZOOM), screen_top_left.y),
            );
        }

        g.unfork();
    }
}

// TODO Move to ezgui
// TODO Actually, remove this concept entirely; it's just a special item in Scroller.
struct Button {
    geom: ScreenRectangle,
    label: Text,
    label_topleft: ScreenPt,
    hovering: bool,
}

impl Button {
    fn new(geom: ScreenRectangle, label: Text, ctx: &EventCtx) -> Button {
        let (width, height) = ctx.canvas.text_dims(&label);
        let label_topleft = ScreenPt::new(
            geom.x1 + (geom.width() - width) / 2.0,
            geom.y1 + (geom.height() - height) / 2.0,
        );
        assert!(label_topleft.x >= 0.0);
        assert!(label_topleft.y >= 0.0);
        Button {
            geom,
            label,
            label_topleft,
            hovering: false,
        }
    }

    fn new_autoheight(top_left: ScreenPt, width: f64, label: Text, ctx: &EventCtx) -> Button {
        let (_, height) = ctx.canvas.text_dims(&label);
        Button::new(
            ScreenRectangle {
                x1: top_left.x,
                x2: top_left.x + width,
                y1: top_left.y,
                y2: top_left.y + height,
            },
            label,
            ctx,
        )
    }

    // True if clicked
    fn event(&mut self, ctx: &mut EventCtx) -> bool {
        if ctx.redo_mouseover() {
            self.hovering = self.geom.contains(ctx.canvas.get_cursor_in_screen_space());
        }
        self.hovering && ctx.input.left_mouse_button_pressed()
    }

    fn draw(&self, g: &mut GfxCtx) {
        g.draw_polygon(
            if self.hovering {
                Color::RED
            } else {
                Color::grey(0.6)
            },
            &Polygon::rectangle_topleft(
                Pt2D::new(self.geom.x1, self.geom.y1),
                Distance::meters(self.geom.width()),
                Distance::meters(self.geom.height()),
            ),
        );
        g.canvas.mark_covered_area(self.geom.clone());
        g.draw_text_at_screenspace_topleft(&self.label, self.label_topleft);
    }
}

struct Scroller<T: Clone + Copy> {
    up_btn: Button,
    down_btn: Button,
    // TODO Maybe the height of each thing; insist that the width is the same for all?
    items: Vec<(T, ScreenDims)>,

    master_topleft: ScreenPt,
    items_topleft: ScreenPt,
    total_dims: ScreenDims,
    hovering_on: Option<usize>,
    bg_color: Color,
    hovering_color: Color,
    // TODO state for actually scrolling down
}

impl<T: Clone + Copy> Scroller<T> {
    fn new(master_topleft: ScreenPt, items: Vec<(T, ScreenDims)>, ctx: &EventCtx) -> Scroller<T> {
        let max_width = items
            .iter()
            .map(|(_, dims)| dims.width)
            .max_by_key(|w| NotNan::new(*w).unwrap())
            .unwrap();
        let up_btn = Button::new_autoheight(
            master_topleft,
            max_width,
            Text::from_line("scroll up".to_string()),
            ctx,
        );
        let items_topleft = ScreenPt::new(master_topleft.x, up_btn.geom.y2);
        let item_height = items.iter().fold(0.0, |sum, (_, dims)| sum + dims.height);
        let down_btn = Button::new_autoheight(
            ScreenPt::new(master_topleft.x, up_btn.geom.y2 + item_height),
            max_width,
            Text::from_line("scroll down".to_string()),
            ctx,
        );

        Scroller {
            items,
            master_topleft,
            items_topleft,
            hovering_on: None,
            // TODO ctx.cs
            bg_color: Color::BLACK.alpha(0.95),
            total_dims: ScreenDims {
                width: max_width,
                height: down_btn.geom.y2 - up_btn.geom.y1,
            },
            up_btn,
            down_btn,
            hovering_color: Color::RED.alpha(0.95),
        }
    }

    // Returns the item selected
    fn event(&mut self, ctx: &mut EventCtx) -> Option<T> {
        if self.up_btn.event(ctx) {
            println!("scroll up");
        }
        if self.down_btn.event(ctx) {
            println!("scroll down");
        }

        if ctx.redo_mouseover() {
            let cursor = ctx.canvas.get_cursor_in_screen_space();
            self.hovering_on = None;
            let mut y1 = self.items_topleft.y;
            for (idx, (_, dims)) in self.items.iter().enumerate() {
                if (ScreenRectangle {
                    x1: self.master_topleft.x,
                    y1,
                    x2: self.master_topleft.x + dims.width,
                    y2: y1 + dims.height,
                }
                .contains(cursor))
                {
                    self.hovering_on = Some(idx);
                    break;
                }
                y1 += dims.height;
            }
        }
        if let Some(idx) = self.hovering_on {
            if ctx.input.left_mouse_button_pressed() {
                return Some(self.items[idx].0);
            }
        }

        None
    }

    // Returns the items to draw, their current top-left point, and their dims.
    fn draw(&self, g: &mut GfxCtx) -> Vec<(T, ScreenPt, ScreenDims)> {
        g.fork_screenspace();
        g.draw_polygon(
            self.bg_color,
            &Polygon::rectangle_topleft(
                Pt2D::new(self.master_topleft.x, self.master_topleft.y),
                Distance::meters(self.total_dims.width),
                Distance::meters(self.total_dims.height),
            ),
        );
        g.canvas.mark_covered_area(ScreenRectangle::top_left(
            self.master_topleft,
            self.total_dims,
        ));

        let mut items_pos = Vec::new();
        let mut y1 = self.items_topleft.y;
        for (idx, (item, dims)) in self.items.iter().enumerate() {
            items_pos.push((*item, ScreenPt::new(self.master_topleft.x, y1), *dims));
            if Some(idx) == self.hovering_on {
                g.draw_polygon(
                    self.hovering_color,
                    &Polygon::rectangle_topleft(
                        Pt2D::new(self.master_topleft.x, y1),
                        Distance::meters(dims.width),
                        Distance::meters(dims.height),
                    ),
                );
            }
            y1 += dims.height;
        }

        self.up_btn.draw(g);
        self.down_btn.draw(g);
        g.unfork();

        items_pos
    }
}
