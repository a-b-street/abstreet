use geom::Distance;
use map_gui::render::{Renderable, OUTLINE_THICKNESS};
use map_model::{
    Direction, EditCmd, EditRoad, LaneID, LaneSpec, LaneType, Road, RoadID, NORMAL_LANE_THICKNESS,
};
use widgetry::{
    lctrl, Choice, Color, ControlState, Drawable, EventCtx, GeomBatch, GfxCtx, HorizontalAlignment,
    Key, Line, Outcome, Panel, State, Text, TextExt, VerticalAlignment, Widget,
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

        let num_edit_cmds_originally = app.primary.map.get_edits().commands.len();
        let top_panel = make_top_panel(ctx, app, num_edit_cmds_originally, true);
        let current_lane = None;
        let main_panel = make_main_panel(ctx, app, app.primary.map.get_r(r), current_lane);
        let highlight_selection = highlight_current_selection(ctx, app, r, current_lane);
        Box::new(RoadEditor {
            r,
            current_lane,
            top_panel,
            main_panel,
            highlight_selection,

            num_edit_cmds_originally,
            redo_stack: Vec::new(),
        })
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
                    return Transition::Pop;
                }
                "undo" => {
                    let mut edits = app.primary.map.get_edits().clone();
                    self.redo_stack.push(edits.commands.pop().unwrap());
                    apply_map_edits(ctx, app, edits);

                    self.current_lane = None;
                    self.main_panel =
                        make_main_panel(ctx, app, app.primary.map.get_r(self.r), self.current_lane);
                    self.highlight_selection =
                        highlight_current_selection(ctx, app, self.r, self.current_lane);
                    self.top_panel = make_top_panel(
                        ctx,
                        app,
                        self.num_edit_cmds_originally,
                        self.redo_stack.is_empty(),
                    );
                }
                "redo" => {
                    let mut edits = app.primary.map.get_edits().clone();
                    edits.commands.push(self.redo_stack.pop().unwrap());
                    apply_map_edits(ctx, app, edits);

                    self.current_lane = None;
                    self.main_panel =
                        make_main_panel(ctx, app, app.primary.map.get_r(self.r), self.current_lane);
                    self.highlight_selection =
                        highlight_current_selection(ctx, app, self.r, self.current_lane);
                    self.top_panel = make_top_panel(
                        ctx,
                        app,
                        self.num_edit_cmds_originally,
                        self.redo_stack.is_empty(),
                    );
                }
                _ => unreachable!(),
            },
            _ => {}
        }

        match self.main_panel.event(ctx) {
            Outcome::Clicked(x) => {
                if let Some(idx) = x.strip_prefix("modify Lane #") {
                    self.current_lane = Some(LaneID(idx.parse().unwrap()));
                    self.main_panel =
                        make_main_panel(ctx, app, app.primary.map.get_r(self.r), self.current_lane);
                    self.highlight_selection =
                        highlight_current_selection(ctx, app, self.r, self.current_lane);
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
                    edits
                        .commands
                        .push(app.primary.map.edit_road_cmd(self.r, |new| {
                            new.lanes_ltr.push(LaneSpec {
                                lt,
                                dir: Direction::Fwd,
                                width: NORMAL_LANE_THICKNESS,
                            });
                        }));
                    apply_map_edits(ctx, app, edits);
                    self.redo_stack.clear();

                    assert!(self.current_lane.is_none());
                    self.current_lane =
                        Some(app.primary.map.get_r(self.r).lanes_ltr().last().unwrap().0);
                    self.main_panel =
                        make_main_panel(ctx, app, app.primary.map.get_r(self.r), self.current_lane);
                    self.highlight_selection =
                        highlight_current_selection(ctx, app, self.r, self.current_lane);
                    self.top_panel = make_top_panel(
                        ctx,
                        app,
                        self.num_edit_cmds_originally,
                        self.redo_stack.is_empty(),
                    );
                } else {
                    unreachable!()
                }
            }
            Outcome::Changed => {
                let speed_limit = self.main_panel.dropdown_value("speed limit");
                if speed_limit != app.primary.map.get_r(self.r).speed_limit {
                    let mut edits = app.primary.map.get_edits().clone();
                    edits
                        .commands
                        .push(app.primary.map.edit_road_cmd(self.r, |new| {
                            new.speed_limit = speed_limit;
                        }));
                    apply_map_edits(ctx, app, edits);
                    self.redo_stack.clear();

                    // Lane IDs don't change
                    self.main_panel =
                        make_main_panel(ctx, app, app.primary.map.get_r(self.r), self.current_lane);
                    self.top_panel = make_top_panel(
                        ctx,
                        app,
                        self.num_edit_cmds_originally,
                        self.redo_stack.is_empty(),
                    );
                } else {
                    let width = self.main_panel.dropdown_value("width");
                    self.modify_current_lane(ctx, app, Some(0), |new, idx| {
                        new.lanes_ltr[idx].width = width;
                    });
                }
            }
            _ => {}
        }

        let mut highlight = self.current_lane;
        if let Some(name) = self.main_panel.currently_hovering() {
            if let Some(idx) = name.strip_prefix("modify Lane #") {
                highlight = Some(LaneID(idx.parse().unwrap()));
            }
        }
        if highlight != self.highlight_selection.0 {
            self.highlight_selection = highlight_current_selection(ctx, app, self.r, highlight);
        }

        if self.current_lane.is_some()
            && ctx.canvas.get_cursor_in_screen_space().is_none()
            && ctx.normal_left_click()
        {
            self.current_lane = None;
            self.main_panel =
                make_main_panel(ctx, app, app.primary.map.get_r(self.r), self.current_lane);
            self.highlight_selection =
                highlight_current_selection(ctx, app, self.r, self.current_lane);
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
            .hotkey(Key::Escape)
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
        ctx.style()
            .btn_plain
            .icon(lane_type_to_icon(lt).unwrap())
            .disabled(Some(lt) == current_lt)
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
    for (id, _, lt) in road.lanes_ltr() {
        // TODO Add direction arrow sometimes
        current_lanes_ltr.push(
            ctx.style()
                .btn_plain
                .icon(lane_type_to_icon(lt).unwrap())
                .disabled(Some(id) == current_lane)
                .image_color(ctx.style().btn_outline.fg, ControlState::Disabled)
                .outline(ctx.style().btn_outline.outline, ControlState::Disabled)
                .build_widget(ctx, format!("modify {}", id)),
        );
    }
    let current_lanes_ltr = Widget::row(current_lanes_ltr);

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
        LaneType::Sidewalk | LaneType::Shoulder => Some("system/assets/meters/pedestrian.svg"),
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
    if choices.iter().all(|x| *x != current_width) {
        choices.push(current_width);
        choices.sort();
    }
    choices
        .into_iter()
        .map(|x| Choice::new(x.to_string(&app.opts.units), x))
        .collect()
}

fn can_reverse(lt: LaneType) -> bool {
    lt == LaneType::Driving || lt == LaneType::Biking || lt == LaneType::Bus
}
