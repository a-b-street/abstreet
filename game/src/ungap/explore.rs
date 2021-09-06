use geom::Distance;
use map_gui::tools::URLManager;
use map_gui::ID;
use map_model::{EditCmd, LaneType};
use widgetry::{
    lctrl, EventCtx, GfxCtx, HorizontalAlignment, Key, Line, Outcome, Panel, State, TextExt,
    VerticalAlignment, Widget,
};

use crate::app::{App, Transition};
use crate::edit::{LoadEdits, RoadEditor, SaveEdits};
use crate::sandbox::gameplay::GameplayMode;
use crate::ungap::{share, Layers, Tab, TakeLayers};

pub struct ExploreMap {
    top_panel: Panel,
    layers: Layers,

    map_edit_key: usize,
}

impl TakeLayers for ExploreMap {
    fn take_layers(self) -> Layers {
        self.layers
    }
}

impl ExploreMap {
    pub fn new_state(ctx: &mut EventCtx, app: &mut App, layers: Layers) -> Box<dyn State<App>> {
        app.opts.show_building_driveways = false;

        if let Err(err) = URLManager::update_url_free_param(
            app.primary
                .map
                .get_name()
                .path()
                .strip_prefix(&abstio::path(""))
                .unwrap()
                .to_string(),
        ) {
            warn!("Couldn't update URL: {}", err);
        }

        Box::new(ExploreMap {
            top_panel: Panel::empty(ctx),
            layers,

            // Start with a bogus value, so we fix up the URL when changing maps
            map_edit_key: usize::MAX,
        })
    }
}

impl State<App> for ExploreMap {
    fn event(&mut self, ctx: &mut EventCtx, app: &mut App) -> Transition {
        // We would normally use Cached, but so many values depend on one key, so this is more
        // clear.
        let key = app.primary.map.get_edits_change_key();
        if self.map_edit_key != key {
            self.map_edit_key = key;
            self.top_panel = make_top_panel(ctx, app);

            if let Err(err) = URLManager::update_url_param(
                "--edits".to_string(),
                app.primary.map.get_edits().edits_name.clone(),
            ) {
                warn!("Couldn't update URL: {}", err);
            }
        }

        if ctx.canvas_movement() {
            if let Err(err) = URLManager::update_url_cam(ctx, app) {
                warn!("Couldn't update URL: {}", err);
            }
        }

        // Only when zoomed in, click to edit a road in detail
        if ctx.canvas.cam_zoom >= app.opts.min_zoom_for_detail {
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
                if ctx.normal_left_click() {
                    return Transition::Push(RoadEditor::new_state_without_lane(ctx, app, r));
                }
            }
        }

        if let Outcome::Clicked(x) = self.top_panel.event(ctx) {
            match x.as_ref() {
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
                    return Transition::Push(share::upload_proposal(ctx, app));
                }
                x => {
                    return Tab::Explore
                        .handle_action::<ExploreMap>(ctx, app, x)
                        .unwrap();
                }
            }
        }

        if let Some(t) = self.layers.event(ctx, app) {
            return t;
        }

        Transition::Keep
    }

    fn draw(&self, g: &mut GfxCtx, app: &App) {
        self.top_panel.draw(g);
        self.layers.draw(g, app);
    }
}

fn make_top_panel(ctx: &mut EventCtx, app: &App) -> Panel {
    let mut file_management = Vec::new();
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
                    total += multiplier * app.primary.map.get_r(*r).center_pts.length();
                }
            }
        }
        total
    };
    if edits.commands.is_empty() {
        file_management.push("Today's network".text_widget(ctx));
    } else {
        file_management.push(Line(&edits.edits_name).into_widget(ctx));
    }
    if false {
        file_management.push(
            Line(format!(
                "{:.1} miles of new bike lanes",
                total_mileage.to_miles()
            ))
            .secondary()
            .into_widget(ctx),
        );
    }
    file_management.push(Widget::row(vec![
        ctx.style()
            .btn_outline
            .text("Open a proposal")
            .hotkey(lctrl(Key::O))
            .build_def(ctx),
        ctx.style()
            .btn_outline
            .text("Save this proposal")
            .hotkey(lctrl(Key::S))
            .disabled(edits.commands.is_empty())
            .build_def(ctx),
    ]));
    if false {
        // TODO Rethink UI of this, probably fold into save dialog
        file_management.push(
            ctx.style()
                .btn_outline
                .text("Share proposal")
                .disabled(!share::UploadedProposals::should_upload_proposal(app))
                .build_def(ctx),
        );
    }
    // TODO Should undo/redo, save, share functionality also live here?

    Panel::new_builder(Widget::col(vec![
        Tab::Explore.make_header(ctx, app),
        Widget::col(file_management).section(ctx),
    ]))
    .aligned(HorizontalAlignment::Left, VerticalAlignment::Top)
    .build(ctx)
}
