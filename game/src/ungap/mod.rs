mod edit;
mod layers;
mod magnifying;
mod nearby;
mod quick_sketch;

use abstutil::prettyprint_usize;
use geom::Distance;
use map_gui::tools::{nice_map_name, CityPicker, PopupMsg};
use widgetry::{
    lctrl, Drawable, EventCtx, GfxCtx, HorizontalAlignment, Key, Line, Outcome, Panel, State, Text,
    Toggle, VerticalAlignment, Widget,
};

use self::layers::{legend, render_network_layer};
use self::magnifying::MagnifyingGlass;
use self::nearby::Nearby;
use crate::app::{App, Transition};
use crate::common::Warping;

pub struct ExploreMap {
    top_panel: Panel,
    legend: Panel,
    magnifying_glass: MagnifyingGlass,
    network_layer: Drawable,
    elevation: bool,
    // TODO Also cache Nearby, but recalculate it after edits
    nearby: Option<Nearby>,

    changelist_key: (String, usize),
}

impl ExploreMap {
    pub fn new_state(ctx: &mut EventCtx, app: &mut App) -> Box<dyn State<App>> {
        app.opts.show_building_driveways = false;
        let edits = app.primary.map.get_edits();

        Box::new(ExploreMap {
            top_panel: make_top_panel(ctx),
            legend: make_legend(ctx, app, false, false),
            magnifying_glass: MagnifyingGlass::new(ctx, true),
            network_layer: render_network_layer(ctx, app),
            elevation: false,
            nearby: None,

            changelist_key: (edits.edits_name.clone(), edits.commands.len()),
        })
    }
}

impl State<App> for ExploreMap {
    fn event(&mut self, ctx: &mut EventCtx, app: &mut App) -> Transition {
        {
            let edits = app.primary.map.get_edits();
            let changelist_key = (edits.edits_name.clone(), edits.commands.len());
            if self.changelist_key != changelist_key {
                self.changelist_key = changelist_key;
                self.network_layer = crate::ungap::render_network_layer(ctx, app);
            }
        }

        ctx.canvas_movement();

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

        if let Some(pt) = ctx.canvas.get_cursor_in_map_space() {
            if ctx.canvas.cam_zoom < app.opts.min_zoom_for_detail && ctx.normal_left_click() {
                return Transition::Push(Warping::new_state(
                    ctx,
                    pt,
                    Some(10.0),
                    None,
                    &mut app.primary,
                ));
            }
        }

        if let Outcome::Clicked(x) = self.top_panel.event(ctx) {
            match x.as_ref() {
                "about A/B Street" => {
                    return Transition::Push(PopupMsg::new_state(ctx, "TODO", vec!["TODO"]));
                }
                "Bike Master Plan" => {
                    return Transition::Push(PopupMsg::new_state(ctx, "TODO", vec!["TODO"]));
                }
                "Edit network" => {
                    return Transition::Push(edit::QuickEdit::new_state(ctx, app));
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
            g.redraw(&self.network_layer);

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
    }
}

fn make_top_panel(ctx: &mut EventCtx) -> Panel {
    Panel::new_builder(Widget::row(vec![
        ctx.style()
            .btn_plain
            .btn()
            .image_path("system/assets/pregame/logo.svg")
            .image_dims(50.0)
            .build_widget(ctx, "about A/B Street"),
        // TODO Tab style?
        ctx.style()
            .btn_solid_primary
            .text("Today")
            .disabled(true)
            .build_def(ctx)
            .centered_vert(),
        ctx.style()
            .btn_solid_primary
            .text("Bike Master Plan")
            .build_def(ctx)
            .centered_vert(),
        ctx.style()
            .btn_solid_primary
            .icon_text("system/assets/tools/pencil.svg", "Edit network")
            .hotkey(lctrl(Key::E))
            .build_def(ctx)
            .centered_vert(),
    ]))
    .aligned(HorizontalAlignment::Center, VerticalAlignment::Top)
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
