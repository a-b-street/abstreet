use crate::ID;
use geom::Distance;
use map_model::{BufferType, EditCmd, LaneID, LaneSpec, LaneType, RoadID};
use widgetry::tools::{PopupMsg, URLManager};
use widgetry::{
    lctrl, Choice, EventCtx, GfxCtx, Key, Line, Outcome, Panel, State, TextExt, Widget,
};

use crate::app::{App, Transition};
use crate::common::{share, RouteSketcher};
use crate::edit::{apply_map_edits, can_edit_lane, LoadEdits, RoadEditor, SaveEdits};
use crate::sandbox::gameplay::GameplayMode;
use crate::ungap::{Layers, Tab, TakeLayers};

pub struct QuickSketch {
    top_panel: Panel,
    layers: Layers,
    route_sketcher: RouteSketcher,

    map_edit_key: usize,
}

impl TakeLayers for QuickSketch {
    fn take_layers(self) -> Layers {
        self.layers
    }
}

impl QuickSketch {
    pub fn new_state(ctx: &mut EventCtx, app: &mut App, layers: Layers) -> Box<dyn State<App>> {
        let mut qs = QuickSketch {
            top_panel: Panel::empty(ctx),
            layers,
            route_sketcher: RouteSketcher::new(app),

            map_edit_key: usize::MAX,
        };
        qs.update_top_panel(ctx, app);
        Box::new(qs)
    }

    fn update_top_panel(&mut self, ctx: &mut EventCtx, app: &App) {
        let mut col = Vec::new();
        if !self.route_sketcher.is_route_started() {
            col.push("Zoom in and click a road to edit in detail".text_widget(ctx));
        }
        col.push(self.route_sketcher.get_widget_to_describe(ctx));

        if self.route_sketcher.is_route_valid() {
            // We're usually replacing an existing panel, except the very first time.
            let default_buffer = if self.top_panel.has_widget("buffer type") {
                self.top_panel.dropdown_value("buffer type")
            } else {
                Some(BufferType::FlexPosts)
            };
            col.push(Widget::row(vec![
                "Protect the new bike lanes?"
                    .text_widget(ctx)
                    .centered_vert(),
                Widget::dropdown(
                    ctx,
                    "buffer type",
                    default_buffer,
                    vec![
                        // TODO Width / cost summary?
                        Choice::new("diagonal stripes", Some(BufferType::Stripes)),
                        Choice::new("flex posts", Some(BufferType::FlexPosts)),
                        Choice::new("planters", Some(BufferType::Planters)),
                        // Omit the others for now
                        Choice::new("no -- just paint", None),
                    ],
                ),
            ]));
            col.push(
                Widget::custom_row(vec![ctx
                    .style()
                    .btn_solid_primary
                    .text("Add bike lanes")
                    .hotkey(Key::Enter)
                    .disabled(!self.route_sketcher.is_route_valid())
                    .build_def(ctx)])
                .evenly_spaced(),
            );
        }

        let proposals = proposal_management(ctx, app).section(ctx);
        self.top_panel = Tab::AddLanes.make_left_panel(
            ctx,
            app,
            Widget::col(vec![Widget::col(col).section(ctx), proposals]),
        );

        // Also manage the URL here, since this is called for every edit
        let map = &app.primary.map;
        let checksum = map.get_edits().get_checksum(map);
        if share::UploadedProposals::load().md5sums.contains(&checksum) {
            URLManager::update_url_param("--edits".to_string(), format!("remote/{}", checksum));
        } else {
            URLManager::update_url_param("--edits".to_string(), map.get_edits().edits_name.clone());
        }
    }
}

impl State<App> for QuickSketch {
    fn event(&mut self, ctx: &mut EventCtx, app: &mut App) -> Transition {
        let key = app.primary.map.get_edits_change_key();
        if self.map_edit_key != key {
            self.map_edit_key = key;
            self.update_top_panel(ctx, app);
        }

        // Only when zoomed in and not drawing a route, click to edit a road in detail
        if !self.route_sketcher.is_route_started() && ctx.canvas.is_zoomed() {
            if ctx.redo_mouseover() {
                app.primary.current_selection =
                    match app.mouseover_unzoomed_roads_and_intersections(ctx) {
                        Some(ID::Road(r)) => Some(r),
                        Some(ID::Lane(l)) => Some(l.road),
                        _ => None,
                    }
                    .and_then(|r| {
                        if app.primary.map.get_r(r).is_light_rail() {
                            None
                        } else {
                            Some(ID::Road(r))
                        }
                    });
            }
            if let Some(ID::Road(r)) = app.primary.current_selection {
                // If it's light rail, a footway, etc, then the first lane should trigger
                // can_edit_lane
                if ctx.normal_left_click() && can_edit_lane(app, LaneID { road: r, offset: 0 }) {
                    return Transition::Push(RoadEditor::new_state_without_lane(ctx, app, r));
                }
            }
        } else {
            app.primary.current_selection = None;
        }

        if let Outcome::Clicked(x) = self.top_panel.event(ctx) {
            match x.as_ref() {
                "Add bike lanes" => {
                    let messages = make_quick_changes(
                        ctx,
                        app,
                        self.route_sketcher.all_roads(app),
                        self.top_panel.dropdown_value("buffer type"),
                    );
                    self.route_sketcher = RouteSketcher::new(app);
                    self.update_top_panel(ctx, app);
                    return Transition::Push(PopupMsg::new_state(ctx, "Changes made", messages));
                }
                "Open a proposal" => {
                    // Dummy mode, just to allow all edits
                    // TODO Actually, should we make one to express that only road edits are
                    // relevant?
                    let mode = GameplayMode::Freeform(app.primary.map.get_name().clone());

                    // TODO Do we want to do SaveEdits first if unsaved_edits()? We have
                    // auto-saving... and after loading an old "untitled proposal", it looks
                    // unsaved.
                    return Transition::Push(LoadEdits::new_state(ctx, app, mode));
                }
                "Save this proposal" => {
                    return Transition::Push(SaveEdits::new_state(
                        ctx,
                        app,
                        format!("Save \"{}\" as", app.primary.map.get_edits().edits_name),
                        false,
                        Some(Transition::Pop),
                        Box::new(|_, _| {}),
                    ));
                }
                "Share proposal" => {
                    return Transition::Push(share::ShareProposal::new_state(ctx, app, "--ungap"));
                }
                x => {
                    // TODO More brittle routing of outcomes.
                    if self.route_sketcher.on_click(x) {
                        self.update_top_panel(ctx, app);
                        return Transition::Keep;
                    }

                    return Tab::AddLanes
                        .handle_action::<QuickSketch>(ctx, app, x)
                        .unwrap();
                }
            }
        }

        if self.route_sketcher.event(ctx, app) {
            self.update_top_panel(ctx, app);
        }

        if let Some(t) = self.layers.event(ctx, app) {
            return t;
        }

        Transition::Keep
    }

    fn draw(&self, g: &mut GfxCtx, app: &App) {
        self.top_panel.draw(g);
        self.layers.draw(g, app);
        self.route_sketcher.draw(g);
    }
}

fn make_quick_changes(
    ctx: &mut EventCtx,
    app: &mut App,
    roads: Vec<RoadID>,
    buffer_type: Option<BufferType>,
) -> Vec<String> {
    // TODO Erasing changes

    let mut edits = app.primary.map.get_edits().clone();
    let mut changed = 0;
    let mut unchanged = 0;
    for r in roads {
        let old = app.primary.map.get_r_edit(r);
        let mut new = old.clone();
        LaneSpec::maybe_add_bike_lanes(
            &mut new.lanes_ltr,
            buffer_type,
            app.primary.map.get_config().driving_side,
        );
        if old == new {
            unchanged += 1;
        } else {
            changed += 1;
            edits.commands.push(EditCmd::ChangeRoad { r, old, new });
        }
    }
    apply_map_edits(ctx, app, edits);

    let mut messages = Vec::new();
    if changed > 0 {
        messages.push(format!("Added bike lanes to {} segments", changed));
    }
    if unchanged > 0 {
        messages.push(format!("Didn't modify {} segments -- the road isn't wide enough, or there's already a bike lane", unchanged));
    }
    messages
}

fn proposal_management(ctx: &mut EventCtx, app: &App) -> Widget {
    let mut col = Vec::new();
    let edits = app.primary.map.get_edits();

    let total_mileage = {
        // Look for the new lanes...
        let mut total = Distance::ZERO;
        // TODO We're assuming the edits have been compressed.
        for cmd in &edits.commands {
            if let EditCmd::ChangeRoad { r, old, new } = cmd {
                let num_before = old
                    .lanes_ltr
                    .iter()
                    .filter(|spec| spec.lt == LaneType::Biking)
                    .count();
                let num_after = new
                    .lanes_ltr
                    .iter()
                    .filter(|spec| spec.lt == LaneType::Biking)
                    .count();
                if num_before != num_after {
                    let multiplier = (num_after as f64) - (num_before) as f64;
                    total += multiplier * app.primary.map.get_r(*r).length();
                }
            }
        }
        total
    };
    if edits.commands.is_empty() {
        col.push("Today's network".text_widget(ctx));
    } else {
        col.push(Line(&edits.edits_name).into_widget(ctx));
    }
    col.push(
        Line(format!(
            "{:.1} miles of new bike lanes",
            total_mileage.to_miles()
        ))
        .secondary()
        .into_widget(ctx),
    );
    col.push(Widget::row(vec![
        ctx.style()
            .btn_outline
            .text("Open a proposal")
            .hotkey(lctrl(Key::O))
            .build_def(ctx),
        ctx.style()
            .btn_outline
            .icon_text("system/assets/tools/save.svg", "Save this proposal")
            .hotkey(lctrl(Key::S))
            .disabled(edits.commands.is_empty())
            .build_def(ctx),
    ]));
    col.push(
        ctx.style()
            .btn_outline
            .text("Share proposal")
            .disabled(edits.commands.is_empty())
            .build_def(ctx),
    );

    Widget::col(col)
}
