use std::collections::{BTreeMap, HashSet};

use map_gui::tools::DrawSimpleRoadLabels;
use map_model::RoadID;
use osm2streets::CrossingType;
use widgetry::mapspace::{DrawCustomUnzoomedShapes, ObjectID, PerZoom, World, WorldOutcome};
use widgetry::{
    Color, ControlState, Drawable, EventCtx, GeomBatch, GfxCtx, Key, Line, Outcome, Panel,
    RewriteColor, State, Text, Widget,
};

use crate::components::{AppwidePanel, BottomPanel, Mode};
use crate::{colors, App, Toggle3Zoomed, Transition};

pub fn svg_path(ct: CrossingType) -> &'static str {
    match ct {
        CrossingType::Signalized => "system/assets/tools/signalized_crossing.svg",
        CrossingType::Unsignalized => "system/assets/tools/unsignalized_crossing.svg",
    }
}

pub struct Crossings {
    appwide_panel: AppwidePanel,
    bottom_panel: Panel,
    world: World<ID>,
    labels: DrawSimpleRoadLabels,
    draw_porosity: Drawable,
    draw_crossings: Toggle3Zoomed,
}

impl Crossings {
    pub fn new_state(ctx: &mut EventCtx, app: &mut App) -> Box<dyn State<App>> {
        let appwide_panel = AppwidePanel::new(ctx, app, Mode::Crossings);
        let contents = make_bottom_panel(ctx, app);
        let bottom_panel = BottomPanel::new(ctx, &appwide_panel, contents);

        // Just force the layers panel to align above the bottom panel
        app.session
            .layers
            .event(ctx, &app.cs, Mode::Crossings, Some(&bottom_panel));

        Box::new(Self {
            appwide_panel,
            bottom_panel,
            world: make_world(ctx, app),
            labels: DrawSimpleRoadLabels::only_major_roads(ctx, app, colors::ROAD_LABEL),
            draw_porosity: draw_porosity(ctx, app),
            draw_crossings: draw_crossings(ctx, app),
        })
    }
}

impl State<App> for Crossings {
    fn event(&mut self, ctx: &mut EventCtx, app: &mut App) -> Transition {
        if let Some(t) =
            self.appwide_panel
                .event(ctx, app, &crate::save::PreserveState::Crossings, help)
        {
            return t;
        }
        if let Some(t) =
            app.session
                .layers
                .event(ctx, &app.cs, Mode::Crossings, Some(&self.bottom_panel))
        {
            return t;
        }
        if let Outcome::Clicked(x) = self.bottom_panel.event(ctx) {
            match x.as_ref() {
                "signalized crossing" => {
                    app.session.crossing_type = CrossingType::Signalized;
                    let contents = make_bottom_panel(ctx, app);
                    self.bottom_panel = BottomPanel::new(ctx, &self.appwide_panel, contents);
                }
                "unsignalized crossing" => {
                    app.session.crossing_type = CrossingType::Unsignalized;
                    let contents = make_bottom_panel(ctx, app);
                    self.bottom_panel = BottomPanel::new(ctx, &self.appwide_panel, contents);
                }
                _ => unreachable!(),
            }
        }

        if let WorldOutcome::ClickedObject(_) = self.world.event(ctx) {
            // TODO Add or remove a crossing
        }

        Transition::Keep
    }

    fn draw(&self, g: &mut GfxCtx, app: &App) {
        self.appwide_panel.draw(g);
        self.bottom_panel.draw(g);
        app.session.layers.draw(g, app);
        g.redraw(&self.draw_porosity);
        self.world.draw(g);
        self.labels.draw(g);
        app.per_map.draw_poi_icons.draw(g);
        self.draw_crossings.draw(g);
    }

    fn recreate(&mut self, ctx: &mut EventCtx, app: &mut App) -> Box<dyn State<App>> {
        Self::new_state(ctx, app)
    }
}

fn help() -> Vec<&'static str> {
    vec![
        "This shows crossings over main roads.",
        "The number of crossings determines the \"porosity\" of areas",
    ]
}

fn boundary_roads(app: &App) -> HashSet<RoadID> {
    let mut result = HashSet::new();
    for info in app.partitioning().all_neighbourhoods().values() {
        for id in &info.block.perimeter.roads {
            result.insert(id.road);
        }
    }
    result
}

fn draw_crossings(ctx: &EventCtx, app: &App) -> Toggle3Zoomed {
    let mut batch = GeomBatch::new();
    let mut low_zoom = DrawCustomUnzoomedShapes::builder();

    let mut icons = BTreeMap::new();
    for ct in [CrossingType::Signalized, CrossingType::Unsignalized] {
        icons.insert(ct, GeomBatch::load_svg(ctx, svg_path(ct)));
    }

    for r in boundary_roads(app) {
        let road = app.per_map.map.get_r(r);
        for (dist, kind) in &road.crossing_nodes {
            // TODO Change style for user modified

            let icon = &icons[&kind];
            if let Ok((pt, angle)) = road.center_pts.dist_along(*dist) {
                let angle = angle.rotate_degs(90.0);
                batch.append(
                    icon.clone()
                        .scale_to_fit_width(road.get_width().inner_meters())
                        .centered_on(pt)
                        .rotate_around_batch_center(angle),
                );

                // TODO Memory intensive
                let icon = icon.clone();
                // TODO They can shrink a bit past their map size
                low_zoom.add_custom(Box::new(move |batch, thickness| {
                    batch.append(
                        icon.clone()
                            .scale_to_fit_width(30.0 * thickness)
                            .centered_on(pt)
                            .rotate_around_batch_center(angle),
                    );
                }));
            }
        }
    }

    let min_zoom_for_detail = 5.0;
    let step_size = 0.1;
    // TODO Ideally we get rid of Toggle3Zoomed and make DrawCustomUnzoomedShapes handle this
    // medium-zoom case.
    Toggle3Zoomed::new(
        batch.build(ctx),
        low_zoom.build(PerZoom::new(min_zoom_for_detail, step_size)),
    )
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
struct ID(RoadID);
impl ObjectID for ID {}

fn make_world(ctx: &mut EventCtx, app: &App) -> World<ID> {
    let map = &app.per_map.map;
    let mut world = World::bounded(map.get_bounds());

    for r in boundary_roads(app) {
        let road = app.per_map.map.get_r(r);
        world
            .add(ID(r))
            .hitbox(road.get_thick_polygon())
            .drawn_in_master_batch()
            .hover_color(colors::HOVER)
            .clickable()
            .build(ctx);
    }

    world.initialize_hover(ctx);
    world
}

fn draw_porosity(ctx: &EventCtx, app: &App) -> Drawable {
    let mut batch = GeomBatch::new();
    for info in app.partitioning().all_neighbourhoods().values() {
        // I haven't seen a single road segment with multiple crossings yet. If it happens, it's
        // likely just a complex intersection and probably shouldn't count as multiple.
        let num_crossings = info
            .block
            .perimeter
            .roads
            .iter()
            .filter(|id| !app.per_map.map.get_r(id.road).crossing_nodes.is_empty())
            .count();
        let color = if num_crossings == 0 {
            *colors::IMPERMEABLE
        } else if num_crossings == 1 {
            *colors::SEMI_PERMEABLE
        } else {
            *colors::POROUS
        };

        batch.push(color.alpha(0.5), info.block.polygon.clone());
    }
    ctx.upload(batch)
}

fn make_bottom_panel(ctx: &mut EventCtx, app: &App) -> Widget {
    let icon = |ct: CrossingType, key: Key, name: &str| {
        let hide_color = Color::hex("#FDDA06");

        ctx.style()
            .btn_solid_primary
            .icon(svg_path(ct))
            .image_color(
                RewriteColor::Change(hide_color, Color::CLEAR),
                ControlState::Default,
            )
            .image_color(
                RewriteColor::Change(hide_color, Color::CLEAR),
                ControlState::Disabled,
            )
            .hotkey(key)
            .disabled(app.session.crossing_type == ct)
            .tooltip_and_disabled({
                let mut txt = Text::new();
                txt.append(Line(name));
                txt.add_line(Line("Click").fg(ctx.style().text_hotkey_color));
                txt.append(Line(" a main road to add or remove a crossing"));
                txt
            })
            .build_widget(ctx, name)
    };

    Widget::row(vec![
        icon(CrossingType::Unsignalized, Key::F1, "unsignalized crossing"),
        icon(CrossingType::Signalized, Key::F2, "signalized crossing"),
    ])
}
