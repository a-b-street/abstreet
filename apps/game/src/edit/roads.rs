use std::collections::HashMap;

use geom::{Bounds, CornerRadii, Distance, Polygon, Pt2D, UnitFmt};
use map_gui::render::{Renderable, OUTLINE_THICKNESS};
use map_gui::ID;
use map_model::{
    BufferType, Direction, EditCmd, EditRoad, LaneID, LaneSpec, LaneType, MapEdits, Road, RoadID,
};
use widgetry::tools::PopupMsg;
use widgetry::{
    lctrl, Choice, Color, ControlState, DragDrop, Drawable, EdgeInsets, EventCtx, GeomBatch,
    GeomBatchStack, GfxCtx, HorizontalAlignment, Image, Key, Line, Outcome, Panel, PersistentSplit,
    Spinner, StackAxis, State, Text, TextExt, VerticalAlignment, Widget, DEFAULT_CORNER_RADIUS,
};

use crate::app::{App, Transition};
use crate::common::Warping;
use crate::edit::zones::ZoneEditor;
use crate::edit::{apply_map_edits, can_edit_lane, speed_limit_choices};

pub struct RoadEditor {
    r: RoadID,
    selected_lane: Option<LaneID>,
    // This is only for hovering on a lane in the map, not for hovering on a lane card.
    hovering_on_lane: Option<LaneID>,
    top_panel: Panel,
    main_panel: Panel,
    fade_irrelevant: Drawable,

    // (cache_key: (selected, hovering), Drawable)
    lane_highlights: ((Option<LaneID>, Option<LaneID>), Drawable),
    // This gets updated during dragging, and is always cleared out when drag-and-drop ends.
    draw_drop_position: Drawable,

    // Undo/redo management
    num_edit_cmds_originally: usize,
    redo_stack: Vec<EditCmd>,
    orig_road_state: EditRoad,
}

impl RoadEditor {
    /// Always starts focused on a certain lane.
    pub fn new_state(ctx: &mut EventCtx, app: &mut App, l: LaneID) -> Box<dyn State<App>> {
        RoadEditor::create(ctx, app, l.road, Some(l))
    }

    pub fn new_state_without_lane(
        ctx: &mut EventCtx,
        app: &mut App,
        r: RoadID,
    ) -> Box<dyn State<App>> {
        RoadEditor::create(ctx, app, r, None)
    }

    fn create(
        ctx: &mut EventCtx,
        app: &mut App,
        r: RoadID,
        selected_lane: Option<LaneID>,
    ) -> Box<dyn State<App>> {
        app.primary.current_selection = None;

        let mut editor = RoadEditor {
            r,
            selected_lane,
            top_panel: Panel::empty(ctx),
            main_panel: Panel::empty(ctx),
            fade_irrelevant: Drawable::empty(ctx),
            lane_highlights: ((None, None), Drawable::empty(ctx)),
            draw_drop_position: Drawable::empty(ctx),
            hovering_on_lane: None,

            num_edit_cmds_originally: app.primary.map.get_edits().commands.len(),
            redo_stack: Vec::new(),
            orig_road_state: app.primary.map.get_r_edit(r),
        };
        editor.recalc_all_panels(ctx, app);
        Box::new(editor)
    }

    fn lane_for_idx(&self, app: &App, idx: usize) -> LaneID {
        app.primary.map.get_r(self.r).lanes[idx].id
    }

    fn modify_current_lane<F: Fn(&mut EditRoad, usize)>(
        &mut self,
        ctx: &mut EventCtx,
        app: &mut App,
        select_new_lane_offset: Option<isize>,
        f: F,
    ) -> Transition {
        let idx = self.selected_lane.unwrap().offset;
        let cmd = app.primary.map.edit_road_cmd(self.r, |new| (f)(new, idx));

        // Special check here -- this invalid state can be reached in many ways.
        if let EditCmd::ChangeRoad { ref new, .. } = cmd {
            let mut parking = 0;
            let mut driving = 0;
            for spec in &new.lanes_ltr {
                if spec.lt == LaneType::Parking {
                    parking += 1;
                } else if spec.lt == LaneType::Driving {
                    driving += 1;
                }
            }
            if parking > 0 && driving == 0 {
                return Transition::Push(PopupMsg::new_state(
                    ctx,
                    "Error",
                    vec!["Parking can't exist without a driving lane to access it."],
                ));
            }
        }

        let mut edits = app.primary.map.get_edits().clone();
        edits.commands.push(cmd);
        apply_map_edits(ctx, app, edits);
        self.redo_stack.clear();

        self.selected_lane = select_new_lane_offset
            .map(|offset| self.lane_for_idx(app, (idx as isize + offset) as usize));
        self.recalc_hovering(ctx, app);

        self.recalc_all_panels(ctx, app);

        Transition::Keep
    }

    fn recalc_all_panels(&mut self, ctx: &mut EventCtx, app: &App) {
        self.main_panel = make_main_panel(
            ctx,
            app,
            app.primary.map.get_r(self.r),
            self.selected_lane,
            self.hovering_on_lane,
        );

        self.top_panel = make_top_panel(
            ctx,
            app,
            self.num_edit_cmds_originally,
            self.redo_stack.is_empty(),
            self.r,
            self.orig_road_state.clone(),
        );

        self.recalc_lane_highlights(ctx, app);

        self.fade_irrelevant = fade_irrelevant(app, self.r).upload(ctx);
    }

    fn recalc_lane_highlights(&mut self, ctx: &mut EventCtx, app: &App) {
        let drag_drop = self.main_panel.find::<DragDrop<LaneID>>("lane cards");
        let selected = drag_drop.selected_value().or(self.selected_lane);
        let hovering = drag_drop.hovering_value().or(self.hovering_on_lane);
        if (selected, hovering) != self.lane_highlights.0 {
            self.lane_highlights = build_lane_highlights(ctx, app, selected, hovering);
        }
    }

    fn compress_edits(&self, app: &App) -> Option<MapEdits> {
        // Compress all of the edits, unless there were 0 or 1 changes
        if app.primary.map.get_edits().commands.len() > self.num_edit_cmds_originally + 2 {
            let mut edits = app.primary.map.get_edits().clone();
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
            return Some(edits);
        }
        None
    }

    // Lane IDs may change with every edit. So immediately after an edit, recalculate mouseover.
    fn recalc_hovering(&mut self, ctx: &EventCtx, app: &mut App) {
        app.recalculate_current_selection(ctx);
        self.hovering_on_lane = match app.primary.current_selection.take() {
            Some(ID::Lane(l)) if can_edit_lane(app, l) => Some(l),
            _ => None,
        };
    }
}

impl State<App> for RoadEditor {
    fn event(&mut self, ctx: &mut EventCtx, app: &mut App) -> Transition {
        ctx.canvas_movement();

        let mut panels_need_recalc = false;

        if let Outcome::Clicked(x) = self.top_panel.event(ctx) {
            match x.as_ref() {
                "Finish" => {
                    if let Some(edits) = self.compress_edits(app) {
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
                "Revert" => {
                    let mut edits = app.primary.map.get_edits().clone();
                    edits.commands.push(EditCmd::ChangeRoad {
                        r: self.r,
                        old: app.primary.map.get_r_edit(self.r),
                        new: EditRoad::get_orig_from_osm(
                            app.primary.map.get_r(self.r),
                            app.primary.map.get_config(),
                        ),
                    });
                    apply_map_edits(ctx, app, edits);

                    self.redo_stack.clear();
                    self.selected_lane = None;
                    self.recalc_hovering(ctx, app);
                    panels_need_recalc = true;
                }
                "undo" => {
                    let mut edits = app.primary.map.get_edits().clone();
                    self.redo_stack.push(edits.commands.pop().unwrap());
                    apply_map_edits(ctx, app, edits);

                    self.selected_lane = None;
                    self.recalc_hovering(ctx, app);
                    panels_need_recalc = true;
                }
                "redo" => {
                    let mut edits = app.primary.map.get_edits().clone();
                    edits.commands.push(self.redo_stack.pop().unwrap());
                    apply_map_edits(ctx, app, edits);

                    self.selected_lane = None;
                    self.recalc_hovering(ctx, app);
                    panels_need_recalc = true;
                }
                "jump to road" => {
                    return Transition::Push(Warping::new_state(
                        ctx,
                        app.primary.canonical_point(ID::Road(self.r)).unwrap(),
                        Some(10.0),
                        Some(ID::Road(self.r)),
                        &mut app.primary,
                    ));
                }
                "Apply to multiple road segments" => {
                    return Transition::Push(
                        crate::edit::multiple_roads::SelectSegments::new_state(
                            ctx,
                            app,
                            self.r,
                            self.orig_road_state.clone(),
                            app.primary.map.get_r_edit(self.r),
                            self.compress_edits(app)
                                .unwrap_or_else(|| app.primary.map.get_edits().clone()),
                        ),
                    );
                }
                _ => unreachable!(),
            }
        }

        match self.main_panel.event(ctx) {
            Outcome::Clicked(x) => {
                if let Some(idx) = x.strip_prefix("modify Lane #") {
                    self.selected_lane = Some(LaneID::decode_u32(idx.parse().unwrap()));
                    panels_need_recalc = true;
                } else if x == "delete lane" {
                    return self.modify_current_lane(ctx, app, None, |new, idx| {
                        new.lanes_ltr.remove(idx);
                    });
                } else if x == "flip direction" {
                    return self.modify_current_lane(ctx, app, Some(0), |new, idx| {
                        new.lanes_ltr[idx].dir = new.lanes_ltr[idx].dir.opposite();
                    });
                } else if let Some(lt) = x.strip_prefix("change to ") {
                    let lt = if lt == "buffer" {
                        self.main_panel.persistent_split_value("change to buffer")
                    } else {
                        LaneType::from_short_name(lt).unwrap()
                    };
                    let width =
                        LaneSpec::typical_lane_widths(lt, &app.primary.map.get_r(self.r).osm_tags)
                            [0]
                        .0;
                    return self.modify_current_lane(ctx, app, Some(0), |new, idx| {
                        new.lanes_ltr[idx].lt = lt;
                        new.lanes_ltr[idx].width = width;
                    });
                } else if let Some(lt) = x.strip_prefix("add ") {
                    let lt = if lt == "buffer" {
                        self.main_panel.persistent_split_value("add buffer")
                    } else {
                        LaneType::from_short_name(lt).unwrap()
                    };

                    // Special check here
                    if lt == LaneType::Parking
                        && app
                            .primary
                            .map
                            .get_r(self.r)
                            .lanes
                            .iter()
                            .all(|l| l.lane_type != LaneType::Driving)
                    {
                        return Transition::Push(PopupMsg::new_state(ctx, "Error", vec!["Add a driving lane first. Parking can't exist without a way to access it."]));
                    }

                    let mut edits = app.primary.map.get_edits().clone();
                    let old = app.primary.map.get_r_edit(self.r);
                    let mut new = old.clone();
                    let idx = LaneSpec::add_new_lane(
                        &mut new.lanes_ltr,
                        lt,
                        &app.primary.map.get_r(self.r).osm_tags,
                        app.primary.map.get_config().driving_side,
                    );
                    edits.commands.push(EditCmd::ChangeRoad {
                        r: self.r,
                        old,
                        new,
                    });
                    apply_map_edits(ctx, app, edits);
                    self.redo_stack.clear();

                    self.selected_lane = Some(self.lane_for_idx(app, idx));
                    self.recalc_hovering(ctx, app);
                    panels_need_recalc = true;
                } else if x == "Access restrictions" {
                    // The RoadEditor maintains an undo/redo stack for a single road, but the
                    // ZoneEditor usually operates on multiple roads. So before we switch over to
                    // it, compress and save the current edits.
                    if let Some(edits) = self.compress_edits(app) {
                        apply_map_edits(ctx, app, edits);
                    }
                    return Transition::Replace(ZoneEditor::new_state(ctx, app, self.r));
                } else {
                    unreachable!()
                }
            }
            Outcome::Changed(x) => match x.as_ref() {
                "speed limit" => {
                    let speed_limit = self.main_panel.dropdown_value("speed limit");

                    let mut edits = app.primary.map.get_edits().clone();
                    let old = app.primary.map.get_r_edit(self.r);
                    let mut new = old.clone();
                    new.speed_limit = speed_limit;
                    edits.commands.push(EditCmd::ChangeRoad {
                        r: self.r,
                        old,
                        new,
                    });
                    apply_map_edits(ctx, app, edits);
                    self.redo_stack.clear();

                    // Keep selecting the same lane, if one was selected
                    self.selected_lane = self
                        .selected_lane
                        .map(|id| self.lane_for_idx(app, id.offset));
                    self.recalc_hovering(ctx, app);
                    panels_need_recalc = true;
                }
                "width preset" => {
                    let width = self.main_panel.dropdown_value("width preset");
                    return self.modify_current_lane(ctx, app, Some(0), |new, idx| {
                        new.lanes_ltr[idx].width = width;
                    });
                }
                "width custom" => {
                    let width = self.main_panel.spinner("width custom");
                    return self.modify_current_lane(ctx, app, Some(0), |new, idx| {
                        new.lanes_ltr[idx].width = width;
                    });
                }
                "lane cards" => {
                    // hovering index changed
                    panels_need_recalc = true;
                }
                "dragging lane cards" => {
                    let (from, to) = self
                        .main_panel
                        .find::<DragDrop<LaneID>>("lane cards")
                        .get_dragging_state()
                        .unwrap();
                    self.draw_drop_position = draw_drop_position(app, self.r, from, to).upload(ctx);
                }
                "change to buffer" => {
                    let lt = self.main_panel.persistent_split_value("change to buffer");
                    app.session.buffer_lane_type = lt;
                    let width =
                        LaneSpec::typical_lane_widths(lt, &app.primary.map.get_r(self.r).osm_tags)
                            [0]
                        .0;
                    return self.modify_current_lane(ctx, app, Some(0), |new, idx| {
                        new.lanes_ltr[idx].lt = lt;
                        new.lanes_ltr[idx].width = width;
                    });
                }
                "add buffer" => {
                    app.session.buffer_lane_type =
                        self.main_panel.persistent_split_value("add buffer");
                }
                _ => unreachable!(),
            },
            Outcome::DragDropReleased(_, old_idx, new_idx) => {
                self.draw_drop_position = Drawable::empty(ctx);

                if old_idx != new_idx {
                    let mut edits = app.primary.map.get_edits().clone();
                    edits
                        .commands
                        .push(app.primary.map.edit_road_cmd(self.r, |new| {
                            let spec = new.lanes_ltr.remove(old_idx);
                            new.lanes_ltr.insert(new_idx, spec);
                        }));
                    apply_map_edits(ctx, app, edits);
                    self.redo_stack.clear();
                }

                self.selected_lane = Some(self.lane_for_idx(app, new_idx));
                self.hovering_on_lane = self.selected_lane;
                panels_need_recalc = true;
            }
            Outcome::Nothing => {}
            _ => debug!("main_panel had unhandled outcome"),
        }

        if self
            .main_panel
            .find::<DragDrop<LaneID>>("lane cards")
            .get_dragging_state()
            .is_some()
        {
            // Even if we drag the lane card into map-space, don't hover on anything in the map.
            self.hovering_on_lane = None;
            // Don't rebuild the panel -- that'll destroy the DragDrop we just started! But do
            // update the outlines
            self.recalc_lane_highlights(ctx, app);
        } else if ctx.redo_mouseover() {
            let prev_hovering_on_lane = self.hovering_on_lane;
            self.recalc_hovering(ctx, app);
            if prev_hovering_on_lane != self.hovering_on_lane {
                panels_need_recalc = true;
            }
        }
        if let Some(l) = self.hovering_on_lane {
            if ctx.normal_left_click() {
                if l.road == self.r {
                    self.selected_lane = Some(l);
                    panels_need_recalc = true;
                } else {
                    // Switch to editing another road, first compressing the edits here if
                    // needed.
                    if let Some(edits) = self.compress_edits(app) {
                        apply_map_edits(ctx, app, edits);
                    }
                    return Transition::Replace(RoadEditor::new_state(ctx, app, l));
                }
            }
        } else if self.selected_lane.is_some()
            && ctx.canvas.get_cursor_in_map_space().is_some()
            && ctx.normal_left_click()
        {
            // Deselect the current lane
            self.selected_lane = None;
            self.hovering_on_lane = None;
            panels_need_recalc = true;
        }

        if panels_need_recalc {
            self.recalc_all_panels(ctx, app);
        }

        Transition::Keep
    }

    fn draw(&self, g: &mut GfxCtx, _: &App) {
        g.redraw(&self.fade_irrelevant);
        g.redraw(&self.lane_highlights.1);
        g.redraw(&self.draw_drop_position);
        self.top_panel.draw(g);
        self.main_panel.draw(g);
    }
}

fn make_top_panel(
    ctx: &mut EventCtx,
    app: &App,
    num_edit_cmds_originally: usize,
    no_redo_cmds: bool,
    r: RoadID,
    orig_road_state: EditRoad,
) -> Panel {
    let map = &app.primary.map;
    let current_state = map.get_r_edit(r);

    Panel::new_builder(Widget::col(vec![
        Widget::row(vec![
            Line(format!("Edit {}", r)).small_heading().into_widget(ctx),
            ctx.style()
                .btn_plain
                .icon("system/assets/tools/location.svg")
                .build_widget(ctx, "jump to road"),
            ctx.style()
                .btn_plain
                .text("+ Apply to multiple")
                .label_color(Color::hex("#4CA7E9"), ControlState::Default)
                .hotkey(Key::M)
                .disabled(current_state == orig_road_state)
                .disabled_tooltip("You have to edit one road segment first, then you can apply the changes to more segments.")
                .build_widget(ctx, "Apply to multiple road segments"),
        ]),
        Widget::row(vec![
            ctx.style()
                .btn_solid_primary
                .text("Finish")
                .hotkey(Key::Enter)
                .build_def(ctx),
            ctx.style()
                .btn_plain
                .icon("system/assets/tools/undo.svg")
                .disabled(map.get_edits().commands.len() == num_edit_cmds_originally)
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
                .btn_plain_destructive
                .text("Revert")
                .disabled(current_state == EditRoad::get_orig_from_osm(map.get_r(r), map.get_config()))
                .build_def(ctx),
            ctx.style()
                .btn_plain
                .text("Cancel")
                .hotkey(Key::Escape)
                .build_def(ctx),
        ]),
    ]))
    .aligned(HorizontalAlignment::Center, VerticalAlignment::Top)
    .build(ctx)
}

fn make_main_panel(
    ctx: &mut EventCtx,
    app: &App,
    road: &Road,
    selected_lane: Option<LaneID>,
    hovering_on_lane: Option<LaneID>,
) -> Panel {
    let map = &app.primary.map;

    let current_lt = selected_lane.map(|l| map.get_l(l).lane_type);

    let current_lts: Vec<LaneType> = road.lanes.iter().map(|l| l.lane_type).collect();

    let lane_types = [
        (LaneType::Driving, Some(Key::D)),
        (LaneType::Biking, Some(Key::B)),
        (LaneType::Bus, Some(Key::T)),
        (LaneType::Sidewalk, Some(Key::S)),
        (LaneType::Parking, Some(Key::P)),
        (LaneType::Construction, Some(Key::C)),
    ];
    // All the buffer lanes are grouped into a PersistentSplit
    let moving_lane_idx = 4;

    let mut lane_type_buttons = HashMap::new();
    for (lane_type, _key) in lane_types {
        let btn = ctx
            .style()
            .btn_outline
            .icon(lane_type_to_icon(lane_type).unwrap());

        lane_type_buttons.insert(lane_type, btn);
    }

    let make_buffer_picker = |ctx, prefix, initial_type| {
        PersistentSplit::widget(
            ctx,
            &format!("{} buffer", prefix),
            initial_type,
            None,
            vec![
                BufferType::Stripes,
                BufferType::FlexPosts,
                BufferType::Planters,
                BufferType::JerseyBarrier,
                BufferType::Curb,
            ]
            .into_iter()
            .map(|buf| {
                let lt = LaneType::Buffer(buf);
                let width = LaneSpec::typical_lane_widths(lt, &road.osm_tags)[0].0;
                Choice::new(
                    format!("{} ({})", lt.short_name(), width.to_string(&app.opts.units)),
                    lt,
                )
            })
            .collect(),
        )
    };

    let add_lane_row = Widget::row(vec![
        "add new".text_widget(ctx).centered_vert(),
        Widget::row({
            let mut row: Vec<Widget> = lane_types
                .iter()
                .map(|(lt, key)| {
                    lane_type_buttons
                        .get(lt)
                        .expect("lane_type button should have been cached")
                        .clone()
                        // When we're modifying an existing lane, hotkeys change the lane, not add
                        // new lanes.
                        .hotkey(if selected_lane.is_none() {
                            key.map(|k| k.into())
                        } else {
                            None
                        })
                        .build_widget(ctx, format!("add {}", lt.short_name()))
                        .centered_vert()
                })
                .collect();
            row.push(make_buffer_picker(ctx, "add", app.session.buffer_lane_type));
            row.insert(moving_lane_idx, Widget::vert_separator(ctx, 40.0));
            row
        }),
    ]);
    let mut drag_drop = DragDrop::new(ctx, "lane cards", StackAxis::Horizontal);

    let road_width = road.get_width();

    for l in &road.lanes {
        let idx = l.id.offset;
        let id = l.id;
        let dir = l.dir;
        let lt = l.lane_type;

        let mut icon_stack = GeomBatchStack::vertical(vec![
            Image::from_path(lane_type_to_icon(lt).unwrap())
                .dims((60.0, 50.0))
                .build_batch(ctx)
                .unwrap()
                .0,
        ]);
        icon_stack.set_spacing(20.0);

        if can_reverse(lt) {
            icon_stack.push(
                Image::from_path(if dir == Direction::Fwd {
                    "system/assets/edit/forwards.svg"
                } else {
                    "system/assets/edit/backwards.svg"
                })
                .dims((30.0, 30.0))
                .build_batch(ctx)
                .unwrap()
                .0,
            );
        }
        let lane_width = map.get_l(id).width;

        icon_stack.push(Text::from(Line(lane_width.to_string(&app.opts.units))).render(ctx));
        let icon_batch = icon_stack.batch();
        let icon_bounds = icon_batch.get_bounds();

        let mut rounding = CornerRadii::zero();
        if idx == 0 {
            rounding.top_left = DEFAULT_CORNER_RADIUS;
        }
        if idx == road.lanes.len() - 1 {
            rounding.top_right = DEFAULT_CORNER_RADIUS;
        }

        let (card_bounds, default_batch, hovering_batch, selected_batch) = {
            let card_batch = |(icon_batch, is_hovering, is_selected)| -> (GeomBatch, Bounds) {
                let road_width_px = 700.0;
                let icon_width = 30.0;
                let lane_ratio_of_road = lane_width / road_width;
                let h_padding = ((road_width_px * lane_ratio_of_road - icon_width) / 2.0).max(2.0);

                Image::from_batch(icon_batch, icon_bounds)
                    // TODO: For selected/hover, rather than change the entire card's background, let's
                    // just add an outline to match the styling of the corresponding lane in the map
                    .bg_color(if is_selected {
                        selected_lane_bg(ctx)
                    } else if is_hovering {
                        selected_lane_bg(ctx).dull(0.3)
                    } else {
                        selected_lane_bg(ctx).dull(0.15)
                    })
                    .color(ctx.style().btn_tab.fg)
                    .dims((30.0, 100.0))
                    .padding(EdgeInsets {
                        top: 32.0,
                        left: h_padding,
                        bottom: 32.0,
                        right: h_padding,
                    })
                    .corner_rounding(rounding)
                    .build_batch(ctx)
                    .unwrap()
            };

            let (mut default_batch, bounds) = card_batch((icon_batch.clone(), false, false));
            let border = {
                let top_left = Pt2D::new(bounds.min_x, bounds.max_y - 2.0);
                let bottom_right = Pt2D::new(bounds.max_x, bounds.max_y);
                Polygon::rectangle_two_corners(top_left, bottom_right).unwrap()
            };
            default_batch.push(ctx.style().section_outline.1.shade(0.2), border);
            let (hovering_batch, _) = card_batch((icon_batch.clone(), true, false));
            let (selected_batch, _) = card_batch((icon_batch, false, true));
            (bounds, default_batch, hovering_batch, selected_batch)
        };

        drag_drop.push_card(
            id,
            card_bounds.into(),
            default_batch,
            hovering_batch,
            selected_batch,
        );
    }
    drag_drop.set_initial_state(selected_lane, hovering_on_lane);

    let modify_lane = if let Some(l) = selected_lane {
        let lane = map.get_l(l);
        Widget::col(vec![
            Widget::row(vec![
                "change to".text_widget(ctx).centered_vert(),
                Widget::row({
                    let mut row: Vec<Widget> = lane_types
                        .iter()
                        .map(|(lt, key)| {
                            let lt = *lt;
                            let mut btn = lane_type_buttons
                                .get(&lt)
                                .expect("lane_type button should have been cached")
                                .clone()
                                .hotkey(key.map(|k| k.into()));

                            if current_lt == Some(lt) {
                                // If the selected lane is already this type, we can't change it. Hopefully no need to
                                // explain this.
                                btn = btn.disabled(true);
                            } else if lt == LaneType::Parking
                                && current_lts
                                    .iter()
                                    .filter(|x| **x == LaneType::Parking)
                                    .count()
                                    == 2
                            {
                                // Max 2 parking lanes per road.
                                //
                                // (I've seen cases in Ballard with angled parking in a median and also parking on both
                                // shoulders. If this happens to be mapped as two adjacent one-way roads, it could
                                // work. But the simulation layer doesn't understand 3 lanes on one road.)
                                btn = btn
                                    .disabled(true)
                                    .disabled_tooltip("This road already has two parking lanes");
                            } else if lt == LaneType::Sidewalk
                                && current_lts.iter().filter(|x| x.is_walkable()).count() == 2
                            {
                                // Max 2 sidewalks or shoulders per road.
                                //
                                // (You could imagine some exceptions in reality, but this assumption of max 2 is
                                // deeply baked into the map model and everything on top of it.)
                                btn = btn
                                    .disabled(true)
                                    .disabled_tooltip("This road already has two sidewalks");
                            }

                            btn.build_widget(ctx, format!("change to {}", lt.short_name()))
                        })
                        .collect();
                    row.push(make_buffer_picker(
                        ctx,
                        "change to",
                        match current_lt {
                            Some(lt @ LaneType::Buffer(_)) => lt,
                            _ => app.session.buffer_lane_type,
                        },
                    ));
                    row.insert(moving_lane_idx, Widget::vert_separator(ctx, 40.0));
                    row
                }),
            ]),
            Widget::row(vec![
                ctx.style()
                    .btn_solid_destructive
                    .icon("system/assets/tools/trash.svg")
                    .disabled(road.lanes.len() == 1)
                    .hotkey(Key::Backspace)
                    .build_widget(ctx, "delete lane")
                    .centered_vert(),
                ctx.style()
                    .btn_plain
                    .text("flip direction")
                    .disabled(!can_reverse(lane.lane_type))
                    .hotkey(Key::F)
                    .build_def(ctx)
                    .centered_vert(),
                Widget::row(vec![
                    Line("Width").secondary().into_widget(ctx).centered_vert(),
                    Widget::dropdown(ctx, "width preset", lane.width, width_choices(app, l)),
                    Spinner::widget_with_custom_rendering(
                        ctx,
                        "width custom",
                        (Distance::meters(0.3), Distance::meters(7.0)),
                        lane.width,
                        Distance::meters(0.1),
                        // Even if the user's settings are set to feet, our step size is in meters, so
                        // just render in meters.
                        Box::new(|x| {
                            x.to_string(&UnitFmt {
                                round_durations: false,
                                metric: true,
                            })
                        }),
                    ),
                ])
                .section(ctx),
            ]),
        ])
    } else {
        Widget::nothing()
    };

    let total_width = {
        let line1 = Text::from_all(vec![
            Line("Total width ").secondary(),
            Line(road_width.to_string(&app.opts.units)),
        ])
        .into_widget(ctx);
        let orig_width = EditRoad::get_orig_from_osm(map.get_r(road.id), map.get_config())
            .lanes_ltr
            .into_iter()
            .map(|spec| spec.width)
            .sum();
        let line2 = ctx
            .style()
            .btn_plain
            .btn()
            .label_styled_text(
                Text::from(match road_width.cmp(&orig_width) {
                    std::cmp::Ordering::Equal => Line("No change").secondary(),
                    std::cmp::Ordering::Less => Line(format!(
                        "- {}",
                        (orig_width - road_width).to_string(&app.opts.units)
                    ))
                    .fg(Color::GREEN),
                    std::cmp::Ordering::Greater => Line(format!(
                        "+ {}",
                        (road_width - orig_width).to_string(&app.opts.units)
                    ))
                    .fg(Color::RED),
                }),
                ControlState::Default,
            )
            .disabled(true)
            .disabled_tooltip("The original road width is an estimate, so any changes might not require major construction.")
            .build_widget(ctx, "changes to total width")
            .align_right();
        Widget::col(vec![line1, line2])
    };

    let road_settings = Widget::row(vec![
        total_width,
        Line("Speed limit")
            .secondary()
            .into_widget(ctx)
            .centered_vert(),
        Widget::dropdown(
            ctx,
            "speed limit",
            road.speed_limit,
            speed_limit_choices(app, Some(road.speed_limit)),
        )
        .centered_vert(),
        ctx.style()
            .btn_outline
            .text("Access restrictions")
            .build_def(ctx)
            .centered_vert(),
    ]);

    Panel::new_builder(
        Widget::custom_col(vec![
            Widget::col(vec![
                road_settings,
                Widget::horiz_separator(ctx, 1.0),
                add_lane_row,
            ])
            .section(ctx)
            .margin_below(16),
            drag_drop
                .into_widget(ctx)
                .bg(ctx.style().text_primary_color.tint(0.3))
                .margin_left(16),
            // We use a sort of "tab" metaphor for the selected lane above and this "edit" section
            modify_lane.padding(16.0).bg(selected_lane_bg(ctx)),
        ])
        .padding_left(16),
    )
    .aligned(HorizontalAlignment::Left, VerticalAlignment::Center)
    // If we're hovering on a lane card, we'll immediately produce Outcome::Changed. Since this
    // usually happens in recalc_all_panels, that's fine -- we'll look up the current lane card
    // there anyway.
    .ignore_initial_events()
    .build_custom(ctx)
}

fn selected_lane_bg(ctx: &EventCtx) -> Color {
    ctx.style().btn_tab.bg_disabled
}

fn build_lane_highlights(
    ctx: &EventCtx,
    app: &App,
    selected_lane: Option<LaneID>,
    hovered_lane: Option<LaneID>,
) -> ((Option<LaneID>, Option<LaneID>), Drawable) {
    let mut batch = GeomBatch::new();
    let map = &app.primary.map;

    let selected_color = selected_lane_bg(ctx);
    let hovered_color = app.cs.selected;

    if let Some(hovered_lane) = hovered_lane {
        batch.push(
            hovered_color,
            app.primary.draw_map.get_l(hovered_lane).get_outline(map),
        );
    }

    if let Some(selected_lane) = selected_lane {
        batch.push(
            selected_color,
            app.primary.draw_map.get_l(selected_lane).get_outline(map),
        );
    }

    ((selected_lane, hovered_lane), ctx.upload(batch))
}

fn lane_type_to_icon(lt: LaneType) -> Option<&'static str> {
    match lt {
        LaneType::Driving => Some("system/assets/edit/driving.svg"),
        LaneType::Parking => Some("system/assets/edit/parking.svg"),
        LaneType::Sidewalk | LaneType::Shoulder => Some("system/assets/edit/sidewalk.svg"),
        LaneType::Biking => Some("system/assets/edit/bike.svg"),
        LaneType::Bus => Some("system/assets/edit/bus.svg"),
        LaneType::SharedLeftTurn => Some("system/assets/map/shared_left_turn.svg"),
        LaneType::Construction => Some("system/assets/edit/construction.svg"),
        LaneType::Buffer(BufferType::Stripes) => Some("system/assets/edit/buffer/stripes.svg"),
        LaneType::Buffer(BufferType::FlexPosts) => Some("system/assets/edit/buffer/flex_posts.svg"),
        LaneType::Buffer(BufferType::Planters) => Some("system/assets/edit/buffer/planters.svg"),
        LaneType::Buffer(BufferType::JerseyBarrier) => {
            Some("system/assets/edit/buffer/jersey_barrier.svg")
        }
        LaneType::Buffer(BufferType::Curb) => Some("system/assets/edit/buffer/curb.svg"),
        // Don't allow creating these yet
        LaneType::LightRail => None,
    }
}

fn width_choices(app: &App, l: LaneID) -> Vec<Choice<Distance>> {
    let lane = app.primary.map.get_l(l);
    let mut choices = LaneSpec::typical_lane_widths(
        lane.lane_type,
        &app.primary.map.get_r(lane.id.road).osm_tags,
    );
    if !choices.iter().any(|(x, _)| *x == lane.width) {
        choices.push((lane.width, "custom"));
    }
    choices.sort();
    choices
        .into_iter()
        .map(|(x, label)| Choice::new(format!("{} - {}", x.to_string(&app.opts.units), label), x))
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

fn fade_irrelevant(app: &App, r: RoadID) -> GeomBatch {
    let map = &app.primary.map;
    let road = map.get_r(r);
    let mut holes = vec![road.get_thick_polygon()];
    for i in [road.src_i, road.dst_i] {
        let i = map.get_i(i);
        holes.push(i.polygon.clone());
    }

    // The convex hull illuminates a bit more of the surrounding area, looks better
    let fade_area = Polygon::with_holes(
        map.get_boundary_polygon().clone().into_ring(),
        vec![Polygon::convex_hull(holes).into_ring()],
    );
    GeomBatch::from(vec![(app.cs.fade_map_dark, fade_area)])
}

fn draw_drop_position(app: &App, r: RoadID, from: usize, to: usize) -> GeomBatch {
    let mut batch = GeomBatch::new();
    if from == to {
        return batch;
    }
    let map = &app.primary.map;
    let road = map.get_r(r);
    let take_num = if from < to { to + 1 } else { to };
    let width = road.lanes.iter().take(take_num).map(|x| x.width).sum();
    if let Ok(pl) = road.shift_from_left_side(width) {
        batch.push(app.cs.selected, pl.make_polygons(OUTLINE_THICKNESS));
    }
    batch
}
