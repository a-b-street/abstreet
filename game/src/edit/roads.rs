use geom::{CornerRadii, Distance};
use map_gui::render::{Renderable, OUTLINE_THICKNESS};
use map_gui::ID;
use map_model::{
    Direction, EditCmd, EditRoad, LaneID, LaneSpec, LaneType, Road, RoadID, NORMAL_LANE_THICKNESS,
};
use widgetry::{
    lctrl, Choice, Color, ControlState, Drawable, EventCtx, GeomBatch, GeomBatchStack, GfxCtx,
    HorizontalAlignment, Image, Key, Line, Outcome, Panel, State, Text, TextExt, VerticalAlignment,
    Widget, DEFAULT_CORNER_RADIUS,
};

use crate::app::{App, Transition};
use crate::edit::{apply_map_edits, speed_limit_choices};

pub struct RoadEditor {
    r: RoadID,
    current_lane: Option<LaneID>,
    top_panel: Panel,
    main_panel: Panel,
    highlight_selection: (Option<LaneID>, Drawable),

    // Undo/redo management
    num_edit_cmds_originally: usize,
    redo_stack: Vec<EditCmd>,
}

impl RoadEditor {
    pub fn new(ctx: &mut EventCtx, app: &mut App, r: RoadID) -> Box<dyn State<App>> {
        app.primary.current_selection = None;

        let mut editor = RoadEditor {
            r,
            current_lane: None,
            top_panel: Panel::empty(ctx),
            main_panel: Panel::empty(ctx),
            highlight_selection: (None, Drawable::empty(ctx)),

            num_edit_cmds_originally: app.primary.map.get_edits().commands.len(),
            redo_stack: Vec::new(),
        };
        editor.recalc_all_panels(ctx, app);
        Box::new(editor)
    }

    fn modify_current_lane<F: Fn(&mut EditRoad, usize)>(
        &mut self,
        ctx: &mut EventCtx,
        app: &mut App,
        select_new_lane_offset: Option<isize>,
        f: F,
    ) {
        let idx = app
            .primary
            .map
            .get_r(self.r)
            .offset(self.current_lane.unwrap());

        let mut edits = app.primary.map.get_edits().clone();
        edits
            .commands
            .push(app.primary.map.edit_road_cmd(self.r, |new| (f)(new, idx)));
        apply_map_edits(ctx, app, edits);
        self.redo_stack.clear();

        self.current_lane = if let Some(offset) = select_new_lane_offset {
            Some(app.primary.map.get_r(self.r).lanes_ltr()[((idx as isize) + offset) as usize].0)
        } else {
            None
        };

        self.recalc_all_panels(ctx, app);
    }

    fn recalc_all_panels(&mut self, ctx: &mut EventCtx, app: &App) {
        self.main_panel =
            make_main_panel(ctx, app, app.primary.map.get_r(self.r), self.current_lane);
        self.highlight_selection = highlight_current_selection(ctx, app, self.r, self.current_lane);
        self.top_panel = make_top_panel(
            ctx,
            app,
            self.num_edit_cmds_originally,
            self.redo_stack.is_empty(),
        );
    }
}

impl State<App> for RoadEditor {
    fn event(&mut self, ctx: &mut EventCtx, app: &mut App) -> Transition {
        ctx.canvas_movement();

        match self.top_panel.event(ctx) {
            Outcome::Clicked(x) => match x.as_ref() {
                "Finish" => {
                    // Compress all of the edits, unless there were 0 or 1 changes
                    let mut edits = app.primary.map.get_edits().clone();
                    if edits.commands.len() > self.num_edit_cmds_originally + 2 {
                        let last_edit = match edits.commands.pop().unwrap() {
                            EditCmd::ChangeRoad { new, .. } => new,
                            _ => unreachable!(),
                        };
                        edits.commands.truncate(self.num_edit_cmds_originally + 1);
                        match edits.commands.last_mut().unwrap() {
                            EditCmd::ChangeRoad { ref mut new, .. } => {
                                *new = last_edit;
                            }
                            _ => unreachable!(),
                        }
                        apply_map_edits(ctx, app, edits);
                    }

                    return Transition::Pop;
                }
                "Cancel" => {
                    let mut edits = app.primary.map.get_edits().clone();
                    if edits.commands.len() != self.num_edit_cmds_originally {
                        edits.commands.truncate(self.num_edit_cmds_originally);
                        apply_map_edits(ctx, app, edits);
                    }
                    return Transition::Pop;
                }
                "undo" => {
                    let mut edits = app.primary.map.get_edits().clone();
                    self.redo_stack.push(edits.commands.pop().unwrap());
                    apply_map_edits(ctx, app, edits);

                    self.current_lane = None;
                    self.recalc_all_panels(ctx, app);
                }
                "redo" => {
                    let mut edits = app.primary.map.get_edits().clone();
                    edits.commands.push(self.redo_stack.pop().unwrap());
                    apply_map_edits(ctx, app, edits);

                    self.current_lane = None;
                    self.recalc_all_panels(ctx, app);
                }
                _ => unreachable!(),
            },
            _ => {}
        }

        match self.main_panel.event(ctx) {
            Outcome::Clicked(x) => {
                if let Some(idx) = x.strip_prefix("modify Lane #") {
                    self.current_lane = Some(LaneID(idx.parse().unwrap()));
                    self.recalc_all_panels(ctx, app);
                } else if x == "delete lane" {
                    self.modify_current_lane(ctx, app, None, |new, idx| {
                        new.lanes_ltr.remove(idx);
                    });
                } else if x == "flip direction" {
                    self.modify_current_lane(ctx, app, Some(0), |new, idx| {
                        new.lanes_ltr[idx].dir = new.lanes_ltr[idx].dir.opposite();
                    });
                } else if x == "move lane left" {
                    self.modify_current_lane(ctx, app, Some(-1), |new, idx| {
                        new.lanes_ltr.swap(idx, idx - 1);
                    });
                } else if x == "move lane right" {
                    self.modify_current_lane(ctx, app, Some(1), |new, idx| {
                        new.lanes_ltr.swap(idx, idx + 1);
                    });
                } else if let Some(lt) = x.strip_prefix("change to ") {
                    let lt = LaneType::from_short_name(lt).unwrap();
                    self.modify_current_lane(ctx, app, Some(0), |new, idx| {
                        new.lanes_ltr[idx].lt = lt;
                    });
                } else if let Some(lt) = x.strip_prefix("add ") {
                    let lt = LaneType::from_short_name(lt).unwrap();

                    let mut edits = app.primary.map.get_edits().clone();
                    let old = app.primary.map.get_r_edit(self.r);
                    let mut new = old.clone();
                    let idx = add_new_lane(&mut new, lt);
                    edits.commands.push(EditCmd::ChangeRoad {
                        r: self.r,
                        old,
                        new,
                    });
                    apply_map_edits(ctx, app, edits);
                    self.redo_stack.clear();

                    assert!(self.current_lane.is_none());
                    self.current_lane = Some(app.primary.map.get_r(self.r).lanes_ltr()[idx].0);
                    self.recalc_all_panels(ctx, app);
                } else {
                    unreachable!()
                }
            }
            Outcome::Changed(x) => match x.as_ref() {
                "speed limit" => {
                    let speed_limit = self.main_panel.dropdown_value("speed limit");
                    let mut edits = app.primary.map.get_edits().clone();
                    edits
                        .commands
                        .push(app.primary.map.edit_road_cmd(self.r, |new| {
                            new.speed_limit = speed_limit;
                        }));
                    apply_map_edits(ctx, app, edits);
                    self.redo_stack.clear();

                    // Lane IDs don't change
                    self.recalc_all_panels(ctx, app);
                }
                "width" => {
                    let width = self.main_panel.dropdown_value("width");
                    self.modify_current_lane(ctx, app, Some(0), |new, idx| {
                        new.lanes_ltr[idx].width = width;
                    });
                }
                _ => unreachable!(),
            },
            _ => {}
        }

        if ctx.canvas.get_cursor_in_map_space().is_some() {
            if ctx.redo_mouseover() {
                app.recalculate_current_selection(ctx);
                if let Some(ID::Lane(l)) = app.primary.current_selection {
                    if app.primary.map.get_l(l).parent != self.r {
                        app.primary.current_selection = None;
                    }
                } else {
                    app.primary.current_selection = None;
                }
            }

            if let Some(ID::Lane(l)) = app.primary.current_selection {
                // TODO Update the main panel to show which lane icon we're hovering on
                if ctx.normal_left_click() {
                    self.current_lane = Some(l);
                    self.recalc_all_panels(ctx, app);
                }
            } else if self.current_lane.is_some() && ctx.normal_left_click() {
                // Deselect the current lane
                self.current_lane = None;
                self.recalc_all_panels(ctx, app);
            }
        } else {
            let mut highlight = self.current_lane;
            if let Some(name) = self.main_panel.currently_hovering() {
                if let Some(idx) = name.strip_prefix("modify Lane #") {
                    highlight = Some(LaneID(idx.parse().unwrap()));
                }
            }
            if highlight != self.highlight_selection.0 {
                self.highlight_selection = highlight_current_selection(ctx, app, self.r, highlight);
            }
        }

        Transition::Keep
    }

    fn draw(&self, g: &mut GfxCtx, _: &App) {
        g.redraw(&self.highlight_selection.1);
        self.top_panel.draw(g);
        self.main_panel.draw(g);
    }
}

fn make_top_panel(
    ctx: &mut EventCtx,
    app: &App,
    num_edit_cmds_originally: usize,
    no_redo_cmds: bool,
) -> Panel {
    Panel::new(Widget::row(vec![
        ctx.style()
            .btn_solid_primary
            .text("Finish")
            .hotkey(Key::Enter)
            .build_def(ctx),
        ctx.style()
            .btn_plain
            .icon("system/assets/tools/undo.svg")
            .disabled(app.primary.map.get_edits().commands.len() == num_edit_cmds_originally)
            .hotkey(lctrl(Key::Z))
            .build_widget(ctx, "undo"),
        ctx.style()
            .btn_plain
            .icon("system/assets/tools/redo.svg")
            .disabled(no_redo_cmds)
            // TODO ctrl+shift+Z!
            .hotkey(lctrl(Key::Y))
            .build_widget(ctx, "redo"),
        ctx.style()
            .btn_plain
            .text("Cancel")
            .hotkey(Key::Escape)
            .build_def(ctx),
    ]))
    .aligned(HorizontalAlignment::Center, VerticalAlignment::Top)
    .build(ctx)
}

fn make_main_panel(
    ctx: &mut EventCtx,
    app: &App,
    road: &Road,
    current_lane: Option<LaneID>,
) -> Panel {
    let map = &app.primary.map;

    let modify_lane = if let Some(l) = current_lane {
        let lane = map.get_l(l);
        let idx = road.offset(l);

        Widget::row(vec![
            ctx.style()
                .btn_solid_destructive
                .icon("system/assets/tools/trash.svg")
                .disabled(road.lanes_ltr().len() == 1)
                .build_widget(ctx, "delete lane"),
            ctx.style()
                .btn_plain
                .text("flip direction")
                .disabled(!can_reverse(lane.lane_type))
                .hotkey(Key::F)
                .build_def(ctx),
            Line("Width").secondary().into_widget(ctx).centered_vert(),
            Widget::dropdown(ctx, "width", lane.width, width_choices(app, l)),
            ctx.style()
                .btn_prev()
                .disabled(idx == 0)
                .hotkey(Key::LeftArrow)
                .build_widget(ctx, "move lane left"),
            ctx.style()
                .btn_next()
                .disabled(idx == road.lanes_ltr().len() - 1)
                .hotkey(Key::RightArrow)
                .build_widget(ctx, "move lane right"),
        ])
    } else {
        Widget::nothing()
    };
    let current_lt = current_lane.map(|l| map.get_l(l).lane_type);

    let current_lts: Vec<LaneType> = road.lanes_ltr().into_iter().map(|(_, _, lt)| lt).collect();
    let mut available_lane_types_row = vec![
        LaneType::Driving,
        LaneType::Biking,
        LaneType::Bus,
        LaneType::Parking,
        LaneType::Construction,
        LaneType::Sidewalk,
    ]
    .into_iter()
    .map(|lt| {
        let disabled = if let Some(current) = current_lt {
            lt == current
        } else {
            // Only 2 parking lanes or walkable lanes total make sense
            if lt == LaneType::Parking
                && current_lts
                    .iter()
                    .filter(|x| **x == LaneType::Parking)
                    .count()
                    == 2
            {
                true
            } else if lt == LaneType::Sidewalk
                && current_lts.iter().filter(|x| x.is_walkable()).count() == 2
            {
                true
            } else {
                false
            }
        };

        ctx.style()
            .btn_plain
            .icon(lane_type_to_icon(lt).unwrap())
            .disabled(disabled)
            .build_widget(
                ctx,
                format!(
                    "{} {}",
                    if current_lane.is_some() {
                        "change to"
                    } else {
                        "add"
                    },
                    lt.short_name()
                ),
            )
    })
    .collect::<Vec<Widget>>();
    available_lane_types_row.insert(
        0,
        if current_lane.is_some() {
            "change to"
        } else {
            "add new"
        }
        .text_widget(ctx)
        .centered_vert(),
    );
    let available_lane_types_row = Widget::row(available_lane_types_row);

    let mut current_lanes_ltr = Vec::new();

    let lanes_ltr = road.lanes_ltr();
    let lanes_len = lanes_ltr.len();
    for (idx, (id, dir, lt)) in lanes_ltr.into_iter().enumerate() {
        let mut stack = GeomBatchStack::vertical(vec![
            Image::from_path(lane_type_to_icon(lt).unwrap())
                .build_batch(ctx)
                .unwrap()
                .0,
        ]);
        stack.spacing(20.0);

        if can_reverse(lt) {
            stack.push(
                Image::from_path(if dir == Direction::Fwd {
                    "system/assets/edit/forwards.svg"
                } else {
                    "system/assets/edit/backwards.svg"
                })
                .build_batch(ctx)
                .unwrap()
                .0,
            );
        }
        let stack_batch = stack.batch();
        let stack_bounds = stack_batch.get_bounds();

        let mut rounding = CornerRadii::zero();
        if idx == 0 {
            rounding.top_left = DEFAULT_CORNER_RADIUS;
            rounding.bottom_left = DEFAULT_CORNER_RADIUS;
        }
        if idx == lanes_len - 1 {
            rounding.top_right = DEFAULT_CORNER_RADIUS;
            rounding.bottom_right = DEFAULT_CORNER_RADIUS;
        }

        current_lanes_ltr.push(
            ctx.style()
                .btn_plain
                .btn()
                .image_batch(stack_batch, stack_bounds)
                .disabled(Some(id) == current_lane)
                .bg_color(ctx.style().section_bg, ControlState::Default)
                .bg_color(ctx.style().section_bg.shade(0.1), ControlState::Hovered)
                .bg_color(ctx.style().btn_solid_primary.bg, ControlState::Disabled)
                .image_color(ctx.style().btn_plain.fg, ControlState::Disabled)
                .image_dims(60.0)
                .padding_top(32.0)
                .padding_bottom(32.0)
                .corner_rounding(rounding)
                .build_widget(ctx, format!("modify {}", id)),
        );
    }

    let current_lanes_ltr =
        Widget::evenly_spaced_row(current_lanes_ltr, 2).bg(Color::hex("#979797"));

    let road_settings = Widget::row(vec![
        Text::from_all(vec![
            Line("Total width ").secondary(),
            Line(road.get_width(map).to_string(&app.opts.units)),
        ])
        .into_widget(ctx)
        .centered_vert(),
        Line("Speed limit")
            .secondary()
            .into_widget(ctx)
            .centered_vert(),
        Widget::dropdown(
            ctx,
            "speed limit",
            road.speed_limit,
            speed_limit_choices(app, Some(road.speed_limit)),
        ),
    ]);

    Panel::new(Widget::col(vec![
        modify_lane,
        available_lane_types_row,
        current_lanes_ltr,
        road_settings,
    ]))
    .aligned(HorizontalAlignment::Left, VerticalAlignment::Center)
    .build(ctx)
}

fn highlight_current_selection(
    ctx: &mut EventCtx,
    app: &App,
    r: RoadID,
    l: Option<LaneID>,
) -> (Option<LaneID>, Drawable) {
    let mut batch = GeomBatch::new();
    let color = Color::hex("#DF8C3D");
    let map = &app.primary.map;

    let road = map.get_r(r);
    batch.push(
        color,
        road.center_pts
            .to_thick_boundary(road.get_width(map), OUTLINE_THICKNESS)
            .unwrap(),
    );

    if let Some(l) = l {
        batch.push(color, app.primary.draw_map.get_l(l).get_outline(map));
    }
    (l, ctx.upload(batch))
}

fn lane_type_to_icon(lt: LaneType) -> Option<&'static str> {
    match lt {
        LaneType::Driving => Some("system/assets/edit/driving.svg"),
        LaneType::Parking => Some("system/assets/edit/parking.svg"),
        LaneType::Sidewalk | LaneType::Shoulder => Some("system/assets/edit/sidewalk.svg"),
        LaneType::Biking => Some("system/assets/edit/bike.svg"),
        LaneType::Bus => Some("system/assets/edit/bus.svg"),
        // TODO Add an icon for this
        LaneType::SharedLeftTurn => None,
        LaneType::Construction => Some("system/assets/edit/construction.svg"),
        // Don't allow creating these yet
        LaneType::LightRail => None,
    }
}

fn width_choices(app: &App, l: LaneID) -> Vec<Choice<Distance>> {
    // TODO Use real standard widths for different types
    let mut choices = vec![
        Distance::meters(1.5),
        Distance::meters(2.0),
        Distance::meters(2.5),
        Distance::meters(3.0),
    ];
    let current_width = app.primary.map.get_l(l).width;
    if !choices.contains(&current_width) {
        choices.push(current_width);
        choices.sort();
    }
    choices
        .into_iter()
        .map(|x| Choice::new(x.to_string(&app.opts.units), x))
        .collect()
}

// TODO We need to automatically fix the direction of sidewalks and parking as we initially place
// them or shift them around. Until then, allow fixing in the UI manually.
fn can_reverse(_: LaneType) -> bool {
    true
}
/*fn can_reverse(lt: LaneType) -> bool {
    lt == LaneType::Driving || lt == LaneType::Biking || lt == LaneType::Bus
}*/

// Place the new lane according to its direction on the outside unless the outside is walkable in
// which case place inside the walkable lane
fn default_outside_lane_placement(road: &mut EditRoad, dir: Direction) -> usize {
    if road.lanes_ltr[0].dir == dir {
        if road.lanes_ltr[0].lt.is_walkable() {
            1
        } else {
            0
        }
    } else {
        if road.lanes_ltr.last().unwrap().lt.is_walkable() {
            road.lanes_ltr.len() - 1
        } else {
            road.lanes_ltr.len()
        }
    }
}

// If there are more lanes of type lt pointing forward, then insert the new one backwards, and vice
// versa
fn determine_lane_dir(road: &mut EditRoad, lt: LaneType, minority: bool) -> Direction {
    if (road
        .lanes_ltr
        .iter()
        .filter(|x| x.dir == Direction::Fwd && x.lt == lt)
        .count() as f64
        / road.lanes_ltr.iter().filter(|x| x.lt == lt).count() as f64)
        <= 0.5
    {
        if minority {
            Direction::Fwd
        } else {
            Direction::Back
        }
    } else {
        if minority {
            Direction::Back
        } else {
            Direction::Fwd
        }
    }
}

// Returns the index where the new lane was inserted
fn add_new_lane(road: &mut EditRoad, lt: LaneType) -> usize {
    let dir = match lt {
        LaneType::Driving => determine_lane_dir(road, lt, true),
        LaneType::Biking | LaneType::Bus | LaneType::Parking | LaneType::Construction => {
            let relevant_lanes: Vec<&LaneSpec> =
                road.lanes_ltr.iter().filter(|x| x.lt == lt).collect();
            if relevant_lanes.len() > 0 {
                // When a lane already exists then default to the direction on the other side of the
                // road
                if relevant_lanes[0].dir == Direction::Fwd {
                    Direction::Back
                } else {
                    Direction::Fwd
                }
            } else {
                // If no lanes exist then default to the majority direction to help deal with one
                // way streets, etc.
                determine_lane_dir(road, lt, false)
            }
        }
        LaneType::Sidewalk => {
            if !road.lanes_ltr[0].lt.is_walkable() {
                road.lanes_ltr[0].dir
            } else {
                road.lanes_ltr.last().unwrap().dir
            }
        }
        _ => unreachable!(),
    };

    let idx = match lt {
        // In the middle (where the direction changes)
        LaneType::Driving => road
            .lanes_ltr
            .windows(2)
            .position(|pair| pair[0].dir != pair[1].dir)
            .map(|x| x + 1)
            .unwrap_or(road.lanes_ltr.len()),
        // Place on the dir side, before any sidewalk
        LaneType::Biking | LaneType::Bus | LaneType::Parking | LaneType::Construction => {
            default_outside_lane_placement(road, dir)
        }
        // Place it where it's missing
        LaneType::Sidewalk => {
            if !road.lanes_ltr[0].lt.is_walkable() {
                0
            } else {
                road.lanes_ltr.len()
            }
        }
        _ => unreachable!(),
    };

    road.lanes_ltr.insert(
        idx,
        LaneSpec {
            lt,
            dir,
            width: NORMAL_LANE_THICKNESS,
        },
    );
    idx
}
