//! This is a tool to experiment with the concept of 15-minute neighborhoods. Can you access your
//! daily needs (like groceries, a cafe, a library) within a 15-minute walk, bike ride, or public
//! transit ride of your home?
//!
//! See https://github.com/dabreegster/abstreet/issues/393 for more context.

use abstutil::prettyprint_usize;
use geom::{Distance, Pt2D};
use map_gui::tools::{amenity_type, nice_map_name, CityPicker, PopupMsg};
use map_gui::{Cached, ID};
use map_model::{Building, BuildingID, PathConstraints};
use widgetry::{
    lctrl, Btn, Checkbox, Color, Drawable, EventCtx, GeomBatch, GfxCtx, HorizontalAlignment, Key,
    Line, Outcome, Panel, RewriteColor, State, Text, TextExt, Transition, VerticalAlignment,
    Widget,
};

use crate::isochrone::Isochrone;
use crate::App;

/// This is the UI state for exploring the isochrone/walkshed from a single building.
pub struct Viewer {
    panel: Panel,
    highlight_start: Drawable,
    isochrone: Isochrone,

    hovering_on_bldg: Cached<HoverKey, HoverOnBuilding>,
}

impl Viewer {
    /// Start with a random building
    pub fn random_start(ctx: &mut EventCtx, app: &App) -> Box<dyn State<App>> {
        let bldgs = app.map.all_buildings();
        let start = bldgs[bldgs.len() / 2].id;
        Viewer::new(ctx, app, start)
    }

    pub fn new(ctx: &mut EventCtx, app: &App, start: BuildingID) -> Box<dyn State<App>> {
        let constraints = PathConstraints::Pedestrian;
        let start = app.map.get_b(start);
        let isochrone = Isochrone::new(ctx, app, start.id, constraints);
        let highlight_start = draw_star(ctx, start.polygon.center());
        let panel = build_panel(ctx, app, start, &isochrone);

        Box::new(Viewer {
            panel,
            highlight_start: highlight_start,
            isochrone,
            hovering_on_bldg: Cached::new(),
        })
    }
}

impl State<App> for Viewer {
    fn event(&mut self, ctx: &mut EventCtx, app: &mut App) -> Transition<App> {
        // Allow panning and zooming
        ctx.canvas_movement();

        if ctx.redo_mouseover() {
            let isochrone = &self.isochrone;
            self.hovering_on_bldg
                .update(HoverOnBuilding::key(ctx, app), |key| {
                    HoverOnBuilding::value(ctx, app, key, isochrone)
                });
            // Also update this to conveniently get an outline drawn. Note we don't want to do this
            // inside the callback above, because it doesn't run when the key becomes None.
            app.current_selection = self.hovering_on_bldg.key().map(|(b, _)| ID::Building(b));
        }

        // Don't call normal_left_click unless we're hovering on something in map-space; otherwise
        // panel.event never sees clicks.
        if let Some((hover_id, _)) = self.hovering_on_bldg.key() {
            if ctx.normal_left_click() {
                let start = app.map.get_b(hover_id);
                self.isochrone = Isochrone::new(ctx, app, start.id, self.isochrone.constraints);
                self.highlight_start = draw_star(ctx, start.polygon.center());
                self.panel = build_panel(ctx, app, start, &self.isochrone);
                // Any previous hover is from the perspective of the old `highlight_start`.
                // Remove it so we don't have a dotted line to the previous isochrone's origin
                self.hovering_on_bldg.clear();
            }
        }

        match self.panel.event(ctx) {
            Outcome::Clicked(x) => match x.as_ref() {
                "change map" => {
                    return Transition::Push(CityPicker::new(
                        ctx,
                        app,
                        Box::new(|ctx, app| {
                            Transition::Multi(vec![
                                Transition::Pop,
                                Transition::Replace(Self::random_start(ctx, app)),
                            ])
                        }),
                    ));
                }
                "close" => {
                    return Transition::Pop;
                }
                "About" => {
                    return Transition::Push(PopupMsg::new(
                        ctx,
                        "15 minute neighborhoods",
                        vec![
                            "What if you could access most of your daily needs with a 15-minute \
                             walk or bike ride from your house?",
                            "Wouldn't it be nice to not rely on a climate unfriendly motor \
                             vehicle and get stuck in traffic for these simple errands?",
                            "Different cities around the world are talking about what design and \
                             policy changes could lead to 15 minute neighborhoods.",
                            "This tool lets you see what commercial amenities are near you right \
                             now, using data from OpenStreetMap.",
                        ],
                    ));
                }
                // If we reach here, we must've clicked one of the buttons for an amenity
                category => {
                    // Describe all of the specific amenities matching this category
                    let mut details = Vec::new();
                    for b in self.isochrone.amenities_reachable.get(category) {
                        let bldg = app.map.get_b(*b);
                        for amenity in &bldg.amenities {
                            if amenity_type(&amenity.amenity_type) == Some(category) {
                                details.push(format!(
                                    "{} ({} away) has {}",
                                    bldg.address,
                                    self.isochrone.time_to_reach_building[&bldg.id],
                                    amenity.names.get(app.opts.language.as_ref())
                                ));
                            }
                        }
                    }
                    return Transition::Push(PopupMsg::new(ctx, category, details));
                }
            },
            Outcome::Changed => {
                let constraints = if self.panel.is_checked("walking / biking") {
                    PathConstraints::Pedestrian
                } else {
                    PathConstraints::Bike
                };
                self.isochrone = Isochrone::new(ctx, app, self.isochrone.start, constraints);
                self.panel = build_panel(
                    ctx,
                    app,
                    app.map.get_b(self.isochrone.start),
                    &self.isochrone,
                );
            }
            _ => {}
        }

        Transition::Keep
    }

    fn draw(&self, g: &mut GfxCtx, _: &App) {
        g.redraw(&self.isochrone.draw);
        g.redraw(&self.highlight_start);
        self.panel.draw(g);
        if let Some(ref hover) = self.hovering_on_bldg.value() {
            g.draw_mouse_tooltip(hover.tooltip.clone());
            g.redraw(&hover.drawn_route);
        }
    }
}

/// Draw a star on the start building.
fn draw_star(ctx: &mut EventCtx, center: Pt2D) -> Drawable {
    ctx.upload(
        GeomBatch::load_svg(ctx, "system/assets/tools/star.svg")
            .centered_on(center)
            .color(RewriteColor::ChangeAll(Color::BLACK)),
    )
}

fn build_panel(ctx: &mut EventCtx, app: &App, start: &Building, isochrone: &Isochrone) -> Panel {
    let mut rows = Vec::new();

    rows.push(Widget::row(vec![
        Line("15-minute neighborhood explorer")
            .small_heading()
            .draw(ctx),
        Btn::close(ctx),
    ]));

    rows.push(Widget::row(vec![
        "Map:".draw_text(ctx),
        Btn::pop_up(ctx, Some(nice_map_name(app.map.get_name()))).build(
            ctx,
            "change map",
            lctrl(Key::L),
        ),
    ]));

    rows.push(
        Text::from_all(vec![
            Line("Starting from: ").secondary(),
            Line(&start.address),
        ])
        .draw(ctx),
    );

    rows.push(
        Text::from_all(vec![
            Line("Estimated population: ").secondary(),
            Line(prettyprint_usize(isochrone.population)),
        ])
        .draw(ctx),
    );

    for (amenity, buildings) in isochrone.amenities_reachable.borrow() {
        rows.push(
            Btn::text_fg(format!("{}: {}", amenity, buildings.len())).build(ctx, *amenity, None),
        );
    }

    // Start of toolbar
    rows.push(Widget::horiz_separator(ctx, 0.3).margin_above(10));

    rows.push(Checkbox::toggle(
        ctx,
        "walking / biking",
        "walking",
        "biking",
        None,
        isochrone.constraints == PathConstraints::Pedestrian,
    ));
    rows.push(Btn::plaintext("About").build_def(ctx, None));

    Panel::new(Widget::col(rows))
        .aligned(HorizontalAlignment::Right, VerticalAlignment::Top)
        .build(ctx)
}

struct HoverOnBuilding {
    tooltip: Text,
    drawn_route: Drawable,
}
/// (building, scale factor)
type HoverKey = (BuildingID, f64);

impl HoverOnBuilding {
    fn key(ctx: &EventCtx, app: &App) -> Option<HoverKey> {
        match app.mouseover_unzoomed_buildings(ctx) {
            Some(ID::Building(b)) => {
                let scale_factor = if ctx.canvas.cam_zoom >= app.opts.min_zoom_for_detail {
                    1.0
                } else {
                    10.0
                };
                Some((b, scale_factor))
            }
            _ => None,
        }
    }

    fn value(
        ctx: &mut EventCtx,
        app: &App,
        key: HoverKey,
        isochrone: &Isochrone,
    ) -> HoverOnBuilding {
        debug!("Calculating route for {:?}", key);

        let (hover_id, scale_factor) = key;
        let mut batch = GeomBatch::new();
        if let Some(polyline) = isochrone
            .path_to(&app.map, hover_id)
            .and_then(|path| path.trace(&app.map))
        {
            let dashed_lines = polyline.dashed_lines(
                Distance::meters(0.75 * scale_factor),
                Distance::meters(1.0 * scale_factor),
                Distance::meters(0.4 * scale_factor),
            );
            batch.extend(Color::BLACK, dashed_lines);
        }

        HoverOnBuilding {
            tooltip: if let Some(time) = isochrone.time_to_reach_building.get(&hover_id) {
                Text::from(Line(format!("{} away", time)))
            } else {
                Text::from(Line("This is more than 15 minutes away"))
            },
            drawn_route: ctx.upload(batch),
        }
    }
}
