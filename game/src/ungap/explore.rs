use geom::{ArrowCap, Distance, PolyLine};
use map_gui::tools::URLManager;
use map_gui::ID;
use map_model::{EditCmd, LaneType};
use widgetry::{lctrl, Color, EventCtx, GfxCtx, Key, Line, Outcome, Panel, State, TextExt, Widget};

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

        URLManager::update_url_free_param(
            app.primary
                .map
                .get_name()
                .path()
                .strip_prefix(&abstio::path(""))
                .unwrap()
                .to_string(),
        );

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

            let map = &app.primary.map;
            let checksum = map.get_edits().get_checksum(map);
            if share::UploadedProposals::load().md5sums.contains(&checksum) {
                URLManager::update_url_param("--edits".to_string(), format!("remote/{}", checksum));
            } else {
                URLManager::update_url_param(
                    "--edits".to_string(),
                    map.get_edits().edits_name.clone(),
                );
            }
        }

        if ctx.canvas_movement() {
            URLManager::update_url_cam(ctx, app.primary.map.get_gps_bounds());
        }

        // Only when zoomed in, click to edit a road in detail
        if ctx.canvas.is_zoomed() {
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
                    return Transition::Push(share::ShareProposal::new_state(ctx, app));
                }
                "Show more layers" => {
                    self.layers.show_panel(ctx, app);
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

        if self.top_panel.currently_hovering() == Some(&"Show more layers".to_string()) {
            g.fork_screenspace();
            if let Ok(pl) = PolyLine::new(vec![
                self.top_panel.center_of("Show more layers").to_pt(),
                self.layers.layer_icon_pos().to_pt(),
            ]) {
                g.draw_polygon(
                    Color::RED,
                    pl.make_arrow(Distance::meters(20.0), ArrowCap::Triangle),
                );
            }
            g.unfork();
        }
    }
}

fn make_top_panel(ctx: &mut EventCtx, app: &App) -> Panel {
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
    // TODO Should undo/redo, save, share functionality also live here?

    col.push(
        ctx.style()
            .btn_plain
            .icon_text("system/assets/tools/layers.svg", "Show more layers")
            .build_def(ctx),
    );

    Tab::Explore.make_left_panel(ctx, app, Widget::col(col))
}
