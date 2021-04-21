use geom::Distance;
use map_gui::render::{Renderable, OUTLINE_THICKNESS};
use map_model::{Direction, LaneID, LaneSpec, LaneType, Road, RoadID, NORMAL_LANE_THICKNESS};
use widgetry::{
    Choice, Color, Drawable, EventCtx, GeomBatch, GfxCtx, HorizontalAlignment, Key, Line, Outcome,
    Panel, State, Text, TextExt, VerticalAlignment, Widget,
};

use crate::app::{App, Transition};
use crate::edit::{apply_map_edits, speed_limit_choices};

pub struct RoadEditor {
    r: RoadID,
    current_lane: Option<LaneID>,
    top_panel: Panel,
    main_panel: Panel,
    highlight_selection: (Option<LaneID>, Drawable),
}

impl RoadEditor {
    pub fn new(ctx: &mut EventCtx, app: &mut App, r: RoadID) -> Box<dyn State<App>> {
        app.primary.current_selection = None;

        let top_panel = Panel::new(Widget::row(vec![ctx
            .style()
            .btn_solid_primary
            .text("Finish")
            .hotkey(Key::Escape)
            .build_def(ctx)]))
        .aligned(HorizontalAlignment::Center, VerticalAlignment::Top)
        .build(ctx);
        let current_lane = None;
        let main_panel = make_main_panel(ctx, app, app.primary.map.get_r(r), current_lane);
        let highlight_selection = highlight_current_selection(ctx, app, r, current_lane);
        Box::new(RoadEditor {
            r,
            current_lane,
            top_panel,
            main_panel,
            highlight_selection,
        })
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
                _ => unreachable!(),
            },
            _ => {}
        }

        match self.main_panel.event(ctx) {
            Outcome::Clicked(x) => {
                if x == "delete lane" {
                    let idx = app
                        .primary
                        .map
                        .get_r(self.r)
                        .offset(self.current_lane.unwrap());

                    let mut edits = app.primary.map.get_edits().clone();
                    edits
                        .commands
                        .push(app.primary.map.edit_road_cmd(self.r, |new| {
                            new.lanes_ltr.remove(idx);
                        }));
                    apply_map_edits(ctx, app, edits);

                    self.current_lane = None;
                    self.main_panel =
                        make_main_panel(ctx, app, app.primary.map.get_r(self.r), self.current_lane);
                    self.highlight_selection =
                        highlight_current_selection(ctx, app, self.r, self.current_lane);
                } else if x == "flip direction" {
                    let idx = app
                        .primary
                        .map
                        .get_r(self.r)
                        .offset(self.current_lane.unwrap());

                    let mut edits = app.primary.map.get_edits().clone();
                    edits
                        .commands
                        .push(app.primary.map.edit_road_cmd(self.r, |new| {
                            new.lanes_ltr[idx].dir = new.lanes_ltr[idx].dir.opposite();
                        }));
                    apply_map_edits(ctx, app, edits);

                    self.current_lane = Some(app.primary.map.get_r(self.r).lanes_ltr()[idx].0);
                    self.main_panel =
                        make_main_panel(ctx, app, app.primary.map.get_r(self.r), self.current_lane);
                    self.highlight_selection =
                        highlight_current_selection(ctx, app, self.r, self.current_lane);
                } else if x == "move lane left" {
                    let idx = app
                        .primary
                        .map
                        .get_r(self.r)
                        .offset(self.current_lane.unwrap());

                    let mut edits = app.primary.map.get_edits().clone();
                    edits
                        .commands
                        .push(app.primary.map.edit_road_cmd(self.r, |new| {
                            new.lanes_ltr.swap(idx, idx - 1);
                        }));
                    apply_map_edits(ctx, app, edits);

                    self.current_lane = Some(app.primary.map.get_r(self.r).lanes_ltr()[idx - 1].0);
                    self.main_panel =
                        make_main_panel(ctx, app, app.primary.map.get_r(self.r), self.current_lane);
                    self.highlight_selection =
                        highlight_current_selection(ctx, app, self.r, self.current_lane);
                } else if x == "move lane right" {
                    let idx = app
                        .primary
                        .map
                        .get_r(self.r)
                        .offset(self.current_lane.unwrap());

                    let mut edits = app.primary.map.get_edits().clone();
                    edits
                        .commands
                        .push(app.primary.map.edit_road_cmd(self.r, |new| {
                            new.lanes_ltr.swap(idx, idx + 1);
                        }));
                    apply_map_edits(ctx, app, edits);

                    self.current_lane = Some(app.primary.map.get_r(self.r).lanes_ltr()[idx + 1].0);
                    self.main_panel =
                        make_main_panel(ctx, app, app.primary.map.get_r(self.r), self.current_lane);
                    self.highlight_selection =
                        highlight_current_selection(ctx, app, self.r, self.current_lane);
                } else if let Some(idx) = x.strip_prefix("modify Lane #") {
                    self.current_lane = Some(LaneID(idx.parse().unwrap()));
                    self.main_panel =
                        make_main_panel(ctx, app, app.primary.map.get_r(self.r), self.current_lane);
                    self.highlight_selection =
                        highlight_current_selection(ctx, app, self.r, self.current_lane);
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

                    assert!(self.current_lane.is_none());
                    self.main_panel =
                        make_main_panel(ctx, app, app.primary.map.get_r(self.r), self.current_lane);
                    self.highlight_selection =
                        highlight_current_selection(ctx, app, self.r, self.current_lane);
                } else if let Some(lt) = x.strip_prefix("change to ") {
                    let lt = LaneType::from_short_name(lt).unwrap();
                    let idx = app
                        .primary
                        .map
                        .get_r(self.r)
                        .offset(self.current_lane.unwrap());

                    let mut edits = app.primary.map.get_edits().clone();
                    edits
                        .commands
                        .push(app.primary.map.edit_road_cmd(self.r, |new| {
                            new.lanes_ltr[idx].lt = lt;
                        }));
                    apply_map_edits(ctx, app, edits);

                    self.current_lane = Some(app.primary.map.get_r(self.r).lanes_ltr()[idx].0);
                    self.main_panel =
                        make_main_panel(ctx, app, app.primary.map.get_r(self.r), self.current_lane);
                    self.highlight_selection =
                        highlight_current_selection(ctx, app, self.r, self.current_lane);
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

                    // Lane IDs don't change
                    self.main_panel =
                        make_main_panel(ctx, app, app.primary.map.get_r(self.r), self.current_lane);
                } else {
                    let width = self.main_panel.dropdown_value("width");
                    let idx = app
                        .primary
                        .map
                        .get_r(self.r)
                        .offset(self.current_lane.unwrap());

                    let mut edits = app.primary.map.get_edits().clone();
                    edits
                        .commands
                        .push(app.primary.map.edit_road_cmd(self.r, |new| {
                            new.lanes_ltr[idx].width = width;
                        }));
                    apply_map_edits(ctx, app, edits);

                    self.current_lane = Some(app.primary.map.get_r(self.r).lanes_ltr()[idx].0);
                    self.main_panel =
                        make_main_panel(ctx, app, app.primary.map.get_r(self.r), self.current_lane);
                    self.highlight_selection =
                        highlight_current_selection(ctx, app, self.r, self.current_lane);
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
                .btn_plain
                .icon("system/assets/tools/trash.svg")
                .disabled(road.lanes_ltr().len() == 1)
                .build_widget(ctx, "delete lane"),
            ctx.style()
                .btn_plain
                .text("flip direction")
                .disabled(!can_reverse(lane.lane_type))
                .build_def(ctx),
            Line("Width").secondary().into_widget(ctx),
            Widget::dropdown(ctx, "width", lane.width, width_choices(app, l)),
            ctx.style()
                .btn_plain
                .text("<")
                .disabled(idx == 0)
                .build_widget(ctx, "move lane left"),
            ctx.style()
                .btn_plain
                .text(">")
                .disabled(idx == road.lanes_ltr().len() - 1)
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
    if current_lane.is_some() {
        available_lane_types_row.insert(0, "change to".text_widget(ctx));
    } else {
        available_lane_types_row.insert(0, "add new".text_widget(ctx));
    }
    let available_lane_types_row = Widget::row(available_lane_types_row);

    let mut current_lanes_ltr = Vec::new();
    for (id, _, lt) in road.lanes_ltr() {
        // TODO Add direction arrow sometimes
        current_lanes_ltr.push(
            ctx.style()
                .btn_plain
                .icon(lane_type_to_icon(lt).unwrap())
                .disabled(Some(id) == current_lane)
                .build_widget(ctx, format!("modify {}", id)),
        );
    }
    let current_lanes_ltr = Widget::row(current_lanes_ltr);

    let road_settings = Widget::row(vec![
        Text::from_all(vec![
            Line("Total width").secondary(),
            Line((2.0 * road.get_half_width(map)).to_string(&app.opts.units)),
        ])
        .into_widget(ctx),
        Line("Speed limit").secondary().into_widget(ctx),
        {
            let mut choices = speed_limit_choices(app);
            if !choices.iter().any(|c| c.data == road.speed_limit) {
                choices.push(Choice::new(
                    road.speed_limit.to_string(&app.opts.units),
                    road.speed_limit,
                ));
            }
            Widget::dropdown(ctx, "speed limit", road.speed_limit, choices)
        },
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
            .to_thick_boundary(2.0 * road.get_half_width(map), OUTLINE_THICKNESS)
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
    if choices.iter().any(|x| *x != current_width) {
        choices.push(current_width);
    }
    choices
        .into_iter()
        .map(|x| Choice::new(x.to_string(&app.opts.units), x))
        .collect()
}

fn can_reverse(lt: LaneType) -> bool {
    lt == LaneType::Driving || lt == LaneType::Biking || lt == LaneType::Bus
}
