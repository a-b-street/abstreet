mod layers;
mod magnifying;
mod nearby;
mod quick_sketch;

use abstutil::prettyprint_usize;
use geom::Distance;
use map_gui::load::FutureLoader;
use map_gui::tools::{nice_map_name, CityPicker, ColorLegend, PopupMsg, URLManager};
use map_gui::ID;
use map_model::{EditCmd, LaneType};
use widgetry::{
    lctrl, Drawable, EventCtx, GfxCtx, HorizontalAlignment, Key, Line, Outcome, Panel, State, Text,
    TextExt, Toggle, VerticalAlignment, Widget,
};

use self::layers::{legend, render_edits, DrawNetworkLayer};
use self::magnifying::MagnifyingGlass;
use self::nearby::Nearby;
use crate::app::{App, Transition};
use crate::common::Warping;
use crate::edit::{LoadEdits, RoadEditor, SaveEdits};
use crate::sandbox::gameplay::GameplayMode;

pub const PROPOSAL_HOST_URL: &str = "http://localhost:8080/v1";

pub struct ExploreMap {
    top_panel: Panel,
    legend: Panel,
    magnifying_glass: MagnifyingGlass,
    network_layer: DrawNetworkLayer,
    edits_layer: Drawable,
    elevation: bool,
    // TODO Also cache Nearby, but recalculate it after edits
    nearby: Option<Nearby>,

    map_edit_key: usize,
}

impl ExploreMap {
    pub fn new_state(ctx: &mut EventCtx, app: &mut App) -> Box<dyn State<App>> {
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
            legend: make_legend(ctx, app, false, false),
            magnifying_glass: MagnifyingGlass::new(ctx),
            network_layer: DrawNetworkLayer::new(),
            edits_layer: Drawable::empty(ctx),
            elevation: false,
            nearby: None,

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
            self.network_layer.clear();
            self.edits_layer = render_edits(ctx, app);
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

        self.magnifying_glass.event(ctx, app);

        if ctx.redo_mouseover() && self.elevation {
            let mut label = Text::new().into_widget(ctx);

            if ctx.canvas.cam_zoom < app.opts.min_zoom_for_detail {
                if let Some(pt) = ctx.canvas.get_cursor_in_map_space() {
                    if let Some((elevation, _)) = app
                        .session
                        .elevation_contours
                        .value()
                        .unwrap()
                        .0
                        .closest_pt(pt, Distance::meters(300.0))
                    {
                        label =
                            Line(format!("{} ft", elevation.to_feet().round())).into_widget(ctx);
                    }
                }
            }
            self.legend.replace(ctx, "current elevation", label);
        }

        // Click to edit a road in detail
        if ctx.redo_mouseover() {
            app.primary.current_selection =
                match app.mouseover_unzoomed_roads_and_intersections(ctx) {
                    Some(ID::Road(r)) => Some(r),
                    Some(ID::Lane(l)) => Some(app.primary.map.get_l(l).parent),
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
                return Transition::Multi(vec![
                    Transition::Push(RoadEditor::new_state_without_lane(ctx, app, r)),
                    Transition::Push(Warping::new_state(
                        ctx,
                        ctx.canvas.get_cursor_in_map_space().unwrap(),
                        Some(10.0),
                        None,
                        &mut app.primary,
                    )),
                ]);
            }
        }

        if let Outcome::Clicked(x) = self.top_panel.event(ctx) {
            match x.as_ref() {
                "about A/B Street" => {
                    return Transition::Push(PopupMsg::new_state(ctx, "TODO", vec!["TODO"]));
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
                    return Transition::Push(share_proposal(ctx, app));
                }
                "Sketch a route" => {
                    app.primary.current_selection = None;
                    return Transition::Push(crate::ungap::quick_sketch::QuickSketch::new_state(
                        ctx, app,
                    ));
                }
                _ => unreachable!(),
            }
        }

        match self.legend.event(ctx) {
            Outcome::Clicked(x) => {
                return Transition::Push(match x.as_ref() {
                    "change map" => CityPicker::new_state(
                        ctx,
                        app,
                        Box::new(|ctx, app| {
                            Transition::Multi(vec![Transition::Pop, Transition::Replace(ExploreMap::new_state(ctx, app))])
                        }),
                    ),
                    // TODO Add physical picture examples
                    "highway" => PopupMsg::new_state(ctx, "Highways", vec!["Unless there's a separate trail (like on the 520 or I90 bridge), highways aren't accessible to biking"]),
                    "major street" => PopupMsg::new_state(ctx, "Major streets", vec!["Arterials have more traffic, but are often where businesses are located"]),
                    "minor street" => PopupMsg::new_state(ctx, "Minor streets", vec!["Local streets have a low volume of traffic and are usually comfortable for biking, even without dedicated infrastructure"]),
                    "trail" => PopupMsg::new_state(ctx, "Trails", vec!["Trails like the Burke Gilman are usually well-separated from vehicle traffic. The space is usually shared between people walking, cycling, and rolling."]),
                    "protected bike lane" => PopupMsg::new_state(ctx, "Protected bike lanes", vec!["Bike lanes separated from vehicle traffic by physical barriers or a few feet of striping"]),
                    "painted bike lane" => PopupMsg::new_state(ctx, "Painted bike lanes", vec!["Bike lanes without any separation from vehicle traffic. Often uncomfortably close to the \"door zone\" of parked cars."]),
                    "Stay Healthy Street / greenway" => PopupMsg::new_state(ctx, "Stay Healthy Streets and neighborhood greenways", vec!["Residential streets with additional signage and light barriers. These are intended to be low traffic, dedicated for people walking and biking."]),
                    // TODO Add URLs
                    "about the elevation data" => PopupMsg::new_state(ctx, "About the elevation data", vec!["Biking uphill next to traffic without any dedicated space isn't fun.", "Biking downhill next to traffic, especially in the door-zone of parked cars, and especially on Seattle's bumpy roads... is downright terrifying.", "", "Note the elevation data is incorrect near bridges.", "Thanks to King County LIDAR for the data, and Eldan Goldenberg for processing it."]),
                    "about the things nearby" => PopupMsg::new_state(ctx, "About the things nearby", vec!["Population daa from ?", "Amenities from OpenStreetMap", "A 1-minute biking buffer around the bike network is shown.", "Note 1 minutes depends on direction, especially with steep hills -- this starts FROM the network."]),
                    _ => unreachable!(),
            });
            }
            Outcome::Changed(x) => match x.as_ref() {
                "elevation" => {
                    self.elevation = self.legend.is_checked("elevation");
                    self.legend = make_legend(ctx, app, self.elevation, self.nearby.is_some());
                    if self.elevation {
                        let name = app.primary.map.get_name().clone();
                        if app.session.elevation_contours.key() != Some(name.clone()) {
                            let mut low = Distance::ZERO;
                            let mut high = Distance::ZERO;
                            for i in app.primary.map.all_intersections() {
                                low = low.min(i.elevation);
                                high = high.max(i.elevation);
                            }
                            // TODO Maybe also draw the uphill arrows on the steepest streets?
                            let value = crate::layer::elevation::make_elevation_contours(
                                ctx, app, low, high,
                            );
                            app.session.elevation_contours.set(name, value);
                        }
                    }
                }
                "things nearby" => {
                    if self.legend.is_checked("things nearby") {
                        let nearby = Nearby::new(ctx, app);
                        let label = Text::from(Line(format!(
                            "{} residents, {} shops",
                            prettyprint_usize(nearby.population),
                            prettyprint_usize(nearby.total_amenities)
                        )))
                        .into_widget(ctx);
                        self.legend.replace(ctx, "nearby info", label);
                        self.nearby = Some(nearby);
                    } else {
                        let label = Text::new().into_widget(ctx);
                        self.legend.replace(ctx, "nearby info", label);
                        self.nearby = None;
                    }
                }
                _ => unreachable!(),
            },
            _ => {}
        }

        Transition::Keep
    }

    fn draw(&self, g: &mut GfxCtx, app: &App) {
        self.top_panel.draw(g);
        self.legend.draw(g);
        if g.canvas.cam_zoom < app.opts.min_zoom_for_detail {
            self.network_layer.draw(g, app);

            if self.elevation {
                if let Some((_, ref draw)) = app.session.elevation_contours.value() {
                    g.redraw(draw);
                }
            }
            if let Some(ref nearby) = self.nearby {
                g.redraw(&nearby.draw_buffer);
            }

            self.magnifying_glass.draw(g, app);
        }
        g.redraw(&self.edits_layer);
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
    file_management.push(
        Line(format!(
            "{:.1} miles of new bike lanes",
            total_mileage.to_miles()
        ))
        .secondary()
        .into_widget(ctx),
    );
    file_management.push(ColorLegend::row(
        ctx,
        *crate::ungap::layers::EDITED_COLOR,
        "changed road",
    ));
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
    // TODO Rethink UI of this, probably fold into save dialog
    file_management.push(
        ctx.style()
            .btn_outline
            .text("Share proposal")
            .disabled(edits.commands.is_empty())
            .build_def(ctx),
    );
    // TODO Should undo/redo, save, share functionality also live here?

    Panel::new_builder(Widget::col(vec![
        Widget::row(vec![
            ctx.style()
                .btn_plain
                .btn()
                .image_path("system/assets/pregame/logo.svg")
                .image_dims(50.0)
                .build_widget(ctx, "about A/B Street"),
            Line("Draw your ideal bike network")
                .small_heading()
                .into_widget(ctx)
                .centered_vert(),
        ]),
        Widget::col(file_management).bg(ctx.style().section_bg),
        Widget::row(vec![
            "Click a road to edit in detail"
                .text_widget(ctx)
                .centered_vert(),
            ctx.style()
                .btn_solid_primary
                .text("Sketch a route")
                .hotkey(Key::S)
                .build_def(ctx),
        ])
        .evenly_spaced(),
    ]))
    .aligned(HorizontalAlignment::Right, VerticalAlignment::Top)
    .build(ctx)
}

fn make_legend(ctx: &mut EventCtx, app: &App, elevation: bool, nearby: bool) -> Panel {
    Panel::new_builder(Widget::col(vec![
        Widget::custom_row(vec![
            Line("Bike Network")
                .small_heading()
                .into_widget(ctx)
                .margin_right(18),
            ctx.style()
                .btn_popup_icon_text(
                    "system/assets/tools/map.svg",
                    nice_map_name(app.primary.map.get_name()),
                )
                .hotkey(lctrl(Key::L))
                .build_widget(ctx, "change map")
                .margin_right(8),
        ]),
        // TODO Looks too close to access restrictions
        legend(ctx, app.cs.unzoomed_highway, "highway"),
        legend(ctx, app.cs.unzoomed_arterial, "major street"),
        legend(ctx, app.cs.unzoomed_residential, "minor street"),
        legend(ctx, *layers::DEDICATED_TRAIL, "trail"),
        legend(ctx, *layers::PROTECTED_BIKE_LANE, "protected bike lane"),
        legend(ctx, *layers::PAINTED_BIKE_LANE, "painted bike lane"),
        legend(ctx, *layers::GREENWAY, "Stay Healthy Street / greenway"),
        // TODO Distinguish door-zone bike lanes?
        // TODO Call out bike turning boxes?
        // TODO Call out bike signals?
        Widget::row(vec![
            Toggle::checkbox(ctx, "elevation", Key::E, elevation),
            ctx.style()
                .btn_plain
                .icon("system/assets/tools/info.svg")
                .build_widget(ctx, "about the elevation data")
                .centered_vert(),
            Text::new()
                .into_widget(ctx)
                .named("current elevation")
                .centered_vert(),
        ]),
        Widget::row(vec![
            Toggle::checkbox(ctx, "things nearby", None, nearby),
            ctx.style()
                .btn_plain
                .icon("system/assets/tools/info.svg")
                .build_widget(ctx, "about the things nearby")
                .centered_vert(),
            Text::new()
                .into_widget(ctx)
                .named("nearby info")
                .centered_vert(),
        ]),
    ]))
    .aligned(HorizontalAlignment::Right, VerticalAlignment::Bottom)
    .build(ctx)
}

fn share_proposal(ctx: &mut EventCtx, app: &App) -> Box<dyn State<App>> {
    let (_, outer_progress_rx) = futures_channel::mpsc::channel(1);
    let (_, inner_progress_rx) = futures_channel::mpsc::channel(1);
    let edits_json =
        abstutil::to_json_terse(&app.primary.map.get_edits().to_permanent(&app.primary.map));
    FutureLoader::<App, String>::new_state(
        ctx,
        Box::pin(async move {
            let uuid =
                abstio::http_post(format!("{}/create", PROPOSAL_HOST_URL), edits_json).await?;
            // TODO I'm so lost in this type magic
            let wrapper: Box<dyn Send + FnOnce(&App) -> String> = Box::new(move |_| uuid);
            Ok(wrapper)
        }),
        outer_progress_rx,
        inner_progress_rx,
        "Uploading proposal",
        Box::new(|ctx, _, result| {
            Transition::Replace(match result {
                Ok(uuid) => PopupMsg::new_state(ctx, "Success", vec![format!("ID is {}", uuid)]),
                Err(err) => PopupMsg::new_state(
                    ctx,
                    "Failure",
                    vec![format!("Couldn't upload proposal: {}", err)],
                ),
            })
        }),
    )
}
