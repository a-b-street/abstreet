use abstutil::Counter;
use map_gui::tools::{ColorNetwork, DrawSimpleRoadLabels};
use widgetry::mapspace::{World, WorldOutcome};
use widgetry::tools::{ChooseSomething, PromptInput};
use widgetry::{
    Choice, Color, DrawBaselayer, Drawable, EventCtx, GfxCtx, Outcome, Panel, State, Widget,
};

use crate::components::{AppwidePanel, BottomPanel, Mode};
use crate::edit::EditMode;
use crate::{colors, pages, render, App, Neighbourhood, NeighbourhoodID, Transition};

pub struct PickArea {
    appwide_panel: AppwidePanel,
    bottom_panel: Panel,
    world: World<NeighbourhoodID>,
    draw_over_roads: Drawable,
    draw_boundary_roads: Drawable,
}

impl PickArea {
    pub fn new_state(ctx: &mut EventCtx, app: &mut App) -> Box<dyn State<App>> {
        map_gui::tools::update_url_map_name(app);

        // Make sure we clear this state if we ever switch neighbourhoods
        if let EditMode::Shortcuts(ref mut maybe_focus) = app.session.edit_mode {
            *maybe_focus = None;
        }
        if let EditMode::FreehandFilters(_) = app.session.edit_mode {
            app.session.edit_mode = EditMode::Filters;
        }

        let (world, draw_over_roads) = (make_world(ctx, app), draw_over_roads(ctx, app));

        let appwide_panel = AppwidePanel::new(ctx, app, Mode::PickArea);
        let bottom_panel = BottomPanel::new(
            ctx,
            &appwide_panel,
            Widget::row(vec![
                ctx.style()
                    .btn_outline
                    .text("Change draw style")
                    .build_def(ctx),
                ctx.style()
                    .btn_outline
                    .text("Manage custom boundaries")
                    .build_def(ctx),
            ]),
        );

        // Just force the layers panel to align above the bottom panel
        app.session
            .layers
            .event(ctx, &app.cs, Mode::PickArea, Some(&bottom_panel));

        if app.per_map.draw_major_road_labels.is_none() {
            app.per_map.draw_major_road_labels = Some(DrawSimpleRoadLabels::only_major_roads(
                ctx,
                app,
                colors::ROAD_LABEL,
            ));
        }

        Box::new(Self {
            appwide_panel,
            bottom_panel,
            world,
            draw_over_roads,
            draw_boundary_roads: if app.session.layers.show_main_roads {
                render::draw_main_roads(ctx, app)
            } else {
                render::draw_boundary_roads(ctx, app)
            },
        })
    }
}

impl State<App> for PickArea {
    fn event(&mut self, ctx: &mut EventCtx, app: &mut App) -> Transition {
        if let Some(t) =
            self.appwide_panel
                .event(ctx, app, &crate::save::PreserveState::PickArea, help)
        {
            return t;
        }
        if let Some(t) =
            app.session
                .layers
                .event(ctx, &app.cs, Mode::PickArea, Some(&self.bottom_panel))
        {
            return t;
        }
        if let Outcome::Clicked(x) = self.bottom_panel.event(ctx) {
            if x == "Change draw style" {
                return change_draw_style(ctx);
            } else if x == "Manage custom boundaries" {
                return manage_custom_boundary(ctx, app);
            } else {
                unreachable!()
            }
        }

        if let WorldOutcome::ClickedObject(id) = self.world.event(ctx) {
            return Transition::Push(pages::DesignLTN::new_state(ctx, app, id));
        }

        Transition::Keep
    }

    fn draw_baselayer(&self) -> DrawBaselayer {
        DrawBaselayer::Custom
    }

    fn draw(&self, g: &mut GfxCtx, app: &App) {
        app.draw_with_layering(g, |g| self.world.draw(g));
        self.draw_over_roads.draw(g);

        self.appwide_panel.draw(g);
        self.bottom_panel.draw(g);
        app.session.layers.draw(g, app);
        self.draw_boundary_roads.draw(g);
        app.per_map.draw_major_road_labels.as_ref().unwrap().draw(g);
        app.per_map.draw_all_filters.draw(g);
        app.per_map.draw_poi_icons.draw(g);
    }

    fn recreate(&mut self, ctx: &mut EventCtx, app: &mut App) -> Box<dyn State<App>> {
        Self::new_state(ctx, app)
    }
}

fn make_world(ctx: &mut EventCtx, app: &App) -> World<NeighbourhoodID> {
    let mut world = World::new();
    let map = &app.per_map.map;
    ctx.loading_screen("render neighbourhoods", |ctx, timer| {
        timer.start_iter(
            "render neighbourhoods",
            app.partitioning().all_neighbourhoods().len(),
        );
        for (id, info) in app.partitioning().all_neighbourhoods() {
            timer.next();
            match app.session.draw_neighbourhood_style {
                PickAreaStyle::Simple => {
                    world
                        .add(*id)
                        .hitbox(info.block.polygon.clone())
                        .draw_color(Color::YELLOW.alpha(if info.suspicious_perimeter_roads {
                            0.1
                        } else {
                            0.2
                        }))
                        .hover_alpha(0.5)
                        .clickable()
                        .build(ctx);
                }
                PickAreaStyle::Cells => {
                    let neighbourhood = Neighbourhood::new(app, *id);
                    let render_cells = crate::draw_cells::RenderCells::new(map, &neighbourhood);
                    let hovered_batch = render_cells.draw_colored_areas();
                    world
                        .add(*id)
                        .hitbox(info.block.polygon.clone())
                        .drawn_in_master_batch()
                        .draw_hovered(hovered_batch)
                        .clickable()
                        .build(ctx);
                }
                PickAreaStyle::Quietness => {
                    let neighbourhood = Neighbourhood::new(app, *id);
                    let (quiet_streets, total_streets) = neighbourhood
                        .shortcuts
                        .quiet_and_total_streets(&neighbourhood);
                    let pct = if total_streets == 0 {
                        0.0
                    } else {
                        1.0 - (quiet_streets as f64 / total_streets as f64)
                    };
                    let color = app.cs.good_to_bad_red.eval(pct);
                    world
                        .add(*id)
                        .hitbox(info.block.polygon.clone())
                        .draw_color(color.alpha(0.5))
                        .hover_color(colors::HOVER)
                        .clickable()
                        .build(ctx);
                }
                PickAreaStyle::Shortcuts => {
                    world
                        .add(*id)
                        .hitbox(info.block.polygon.clone())
                        // Slight lie, because draw_over_roads has to be drawn after the World
                        .drawn_in_master_batch()
                        .hover_color(colors::HOVER)
                        .clickable()
                        .build(ctx);
                }
            }
        }
    });
    world
}

fn draw_over_roads(ctx: &mut EventCtx, app: &App) -> Drawable {
    if app.session.draw_neighbourhood_style != PickAreaStyle::Shortcuts {
        return Drawable::empty(ctx);
    }

    let mut count_per_road = Counter::new();
    let mut count_per_intersection = Counter::new();

    for id in app.partitioning().all_neighbourhoods().keys() {
        let neighbourhood = Neighbourhood::new(app, *id);
        count_per_road.extend(neighbourhood.shortcuts.count_per_road);
        count_per_intersection.extend(neighbourhood.shortcuts.count_per_intersection);
    }

    // TODO It's a bit weird to draw one heatmap covering streets in every neighbourhood. The
    // shortcuts are calculated per neighbourhood, but now we're showing them all together, as if
    // it's the impact prediction mode using a demand model.
    let mut colorer = ColorNetwork::no_fading(app);
    colorer.ranked_roads(count_per_road, &app.cs.good_to_bad_red);
    colorer.ranked_intersections(count_per_intersection, &app.cs.good_to_bad_red);
    colorer.build(ctx).unzoomed
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum PickAreaStyle {
    Simple,
    Cells,
    Quietness,
    Shortcuts,
}

fn help() -> Vec<&'static str> {
    vec![
        "Basic map navigation: click and drag to pan, swipe or scroll to zoom",
        "",
        "Click a neighbourhood to analyze it. You can adjust boundaries there.",
    ]
}

fn change_draw_style(ctx: &mut EventCtx) -> Transition {
    Transition::Push(ChooseSomething::new_state(
        ctx,
        "Change draw style",
        vec![
            Choice::new("default", PickAreaStyle::Simple),
            Choice::new("show cells when you hover on an area", PickAreaStyle::Cells),
            Choice::new(
                "color areas by how much shortcutting they have",
                PickAreaStyle::Quietness,
            ),
            Choice::new("show shortcuts through all areas", PickAreaStyle::Shortcuts),
        ],
        Box::new(move |choice, _, app| {
            app.session.draw_neighbourhood_style = choice;
            Transition::Multi(vec![Transition::Pop, Transition::Recreate])
        }),
    ))
}

fn manage_custom_boundary(ctx: &mut EventCtx, app: &App) -> Transition {
    let mut choices = vec![Choice::new("Create new", None)];
    for (id, custom) in &app.partitioning().custom_boundaries {
        choices.push(Choice::new(&custom.name, Some(*id)));
    }

    Transition::Push(ChooseSomething::new_state(
        ctx,
        "Manage custom boundaries",
        choices,
        Box::new(move |choice, ctx, app| {
            if let Some(id) = choice {
                Transition::Clear(vec![pages::DesignLTN::new_state(ctx, app, id)])
            } else {
                Transition::Replace(PromptInput::new_state(
                    ctx,
                    "Name the custom boundary",
                    String::new(),
                    Box::new(|name, ctx, app| {
                        Transition::Clear(vec![pages::FreehandBoundary::blank(ctx, app, name)])
                    }),
                ))
            }
        }),
    ))
}
