use std::collections::{BTreeMap, BTreeSet};

use abstutil::Timer;
use geom::{Duration, UnitFmt};
use map_gui::tools::DrawSimpleRoadLabels;
use map_model::{BuildingID, PathConstraints, PathRequest, Pathfinder};
use synthpop::TripEndpoint;
use widgetry::mapspace::{ObjectID, World, WorldOutcome};
use widgetry::tools::{ColorLegend, ColorScale};
use widgetry::{
    Color, Drawable, EventCtx, GeomBatch, GfxCtx, Line, Outcome, Panel, State, Text, TextExt,
    Widget,
};

use crate::components::{AppwidePanel, BottomPanel, Mode};
use crate::render::colors;
use crate::save::PreserveState;
use crate::{pages, render, App, Neighbourhood, NeighbourhoodID, Transition};

pub struct PerResidentImpact {
    appwide_panel: AppwidePanel,
    bottom_panel: Panel,
    world: World<Obj>,
    labels: DrawSimpleRoadLabels,
    neighbourhood: Neighbourhood,
    fade_irrelevant: Drawable,
    cell_outline: Drawable,
    buildings_inside: BTreeSet<BuildingID>,
    // Expensive to calculate
    preserve_state: PreserveState,

    pathfinder_before: Pathfinder,
    pathfinder_after: Pathfinder,

    current_target: Option<BuildingID>,
    // Time from a building to current_target, (before, after)
    times_from_building: BTreeMap<BuildingID, (Duration, Duration)>,
    compare_routes: Option<(BuildingID, Drawable)>,
}

impl PerResidentImpact {
    pub fn new_state(
        ctx: &mut EventCtx,
        app: &App,
        id: NeighbourhoodID,
        current_target: Option<BuildingID>,
    ) -> Box<dyn State<App>> {
        let map = &app.per_map.map;
        let appwide_panel = AppwidePanel::new(ctx, app, Mode::PerResidentImpact);

        let neighbourhood = Neighbourhood::new(app, id);
        let fade_irrelevant = neighbourhood.fade_irrelevant(ctx, app);
        let mut label_roads = neighbourhood.perimeter_roads.clone();
        label_roads.extend(neighbourhood.interior_roads.clone());
        let labels = DrawSimpleRoadLabels::new(
            ctx,
            app,
            colors::LOCAL_ROAD_LABEL,
            Box::new(move |r| label_roads.contains(&r.id)),
        );

        let mut buildings_inside = BTreeSet::new();
        for b in map.all_buildings() {
            if neighbourhood
                .boundary_polygon
                .contains_pt(b.polygon.center())
            {
                buildings_inside.insert(b.id);
            }
        }

        // It's a subtle effect, but maybe useful to see
        let render_cells = render::RenderCells::new(map, &neighbourhood);
        let cell_outline = render_cells.draw_island_outlines();

        // Depending on the number of buildings_inside, Dijkstra may be faster, but this seems fast
        // enough so far
        let (pathfinder_before, pathfinder_after) =
            ctx.loading_screen("prepare per-resident impact", |_, timer| {
                // TODO Can we share with RoutePlanner maybe?
                timer.start("prepare pathfinding before changes");
                let pathfinder_before = Pathfinder::new_ch(
                    map,
                    app.per_map.routing_params_before_changes.clone(),
                    vec![PathConstraints::Car],
                    timer,
                );
                timer.stop("prepare pathfinding before changes");

                timer.start("prepare pathfinding after changes");
                let mut params = map.routing_params().clone();
                app.edits().update_routing_params(&mut params);
                let pathfinder_after =
                    Pathfinder::new_ch(map, params, vec![PathConstraints::Car], timer);
                timer.stop("prepare pathfinding after changes");

                (pathfinder_before, pathfinder_after)
            });

        let mut state = Self {
            appwide_panel,
            bottom_panel: Panel::empty(ctx),
            world: World::new(),
            labels,
            neighbourhood,
            fade_irrelevant,
            cell_outline: cell_outline.upload(ctx),
            buildings_inside,
            preserve_state: PreserveState::PerResidentImpact(
                app.partitioning().neighbourhood_to_blocks(id),
                current_target,
            ),

            pathfinder_before,
            pathfinder_after,

            current_target,
            times_from_building: BTreeMap::new(),
            compare_routes: None,
        };
        state.update(ctx, app);
        Box::new(state)
    }

    fn update(&mut self, ctx: &mut EventCtx, app: &App) {
        ctx.loading_screen("calculate per-building impacts", |_, timer| {
            self.recalculate_times(app, timer);
        });
        // We should only ever get slower. If there's no change anywhere, use 1s to avoid division
        // by zero
        let max_change = self
            .times_from_building
            .values()
            .map(|(before, after)| *after - *before)
            .max()
            .unwrap_or(Duration::ZERO)
            .max(Duration::seconds(1.0));

        let scale = ColorScale(vec![Color::CLEAR, Color::RED]);
        let mut row = vec![
            ctx.style()
                .btn_outline
                .text("Back")
                .build_def(ctx)
                .centered_vert(),
            Widget::vertical_separator(ctx),
        ];
        if self.current_target.is_none() {
            row.push(
                "Click a building outside the neighbourhood to see driving times there"
                    .text_widget(ctx)
                    .centered_vert(),
            );
        } else {
            row.extend(vec![
                "The time to drive from the neighbourhood to this destination changes:"
                    .text_widget(ctx)
                    .centered_vert(),
                ColorLegend::gradient(
                    ctx,
                    &scale,
                    vec!["0", &max_change.to_string(&UnitFmt::metric())],
                )
                .centered_vert(),
                ColorLegend::row(ctx, *colors::PLAN_ROUTE_BEFORE, "before changes"),
                ColorLegend::row(ctx, *colors::PLAN_ROUTE_AFTER, "after changes"),
            ]);
        }
        self.bottom_panel = BottomPanel::new(ctx, &self.appwide_panel, Widget::row(row));

        let map = &app.per_map.map;
        self.world = World::new();

        for b in map.all_buildings() {
            if let Some((before, after)) = self.times_from_building.get(&b.id) {
                let color = scale.eval((*after - *before) / max_change);
                let mut txt = Text::from(if before == after {
                    format!("No change -- {before}")
                } else {
                    format!(
                        "{} slower -- {before} before this proposal, {after} after",
                        *after - *before
                    )
                });
                if before != after {
                    txt.add_line(Line("Click").fg(ctx.style().text_hotkey_color));
                    txt.append(Line(" to investigate"));
                }

                self.world
                    .add(Obj::Building(b.id))
                    .hitbox(b.polygon.clone())
                    .draw_color(color)
                    .hover_color(colors::HOVER)
                    .tooltip(txt)
                    .clickable()
                    .build(ctx);
            } else {
                self.world
                    .add(Obj::Building(b.id))
                    .hitbox(b.polygon.clone())
                    .drawn_in_master_batch()
                    .hover_color(colors::HOVER)
                    .clickable()
                    .build(ctx);
            }
        }
        self.world.initialize_hover(ctx);

        if let Some(b) = self.current_target {
            self.world.draw_master_batch(
                ctx,
                GeomBatch::load_svg(ctx, "system/assets/tools/star.svg")
                    .centered_on(map.get_b(b).polygon.center()),
            );
        }
    }

    fn recalculate_times(&mut self, app: &App, timer: &mut Timer) {
        self.times_from_building.clear();
        self.compare_routes = None;
        let target = if let Some(b) = self.current_target {
            b
        } else {
            return;
        };

        let map = &app.per_map.map;

        let requests: Vec<(BuildingID, PathRequest)> = self
            .buildings_inside
            .iter()
            .filter_map(|b| {
                PathRequest::between_buildings(map, *b, target, PathConstraints::Car)
                    .map(|req| (*b, req))
            })
            .collect();

        // For each request, calculate the time for each
        for (b, before, after) in timer.parallelize("calculate routes", requests, |(b, req)| {
            (
                b,
                self.pathfinder_before
                    .pathfind_v2(req.clone(), map)
                    .map(|p| p.get_cost()),
                self.pathfinder_after
                    .pathfind_v2(req.clone(), map)
                    .map(|p| p.get_cost()),
            )
        }) {
            if let (Some(before), Some(after)) = (before, after) {
                self.times_from_building.insert(b, (before, after));
            }
        }
    }

    fn compare_routes(&self, ctx: &EventCtx, app: &App, from: BuildingID) -> Option<Drawable> {
        if !self.buildings_inside.contains(&from) {
            return None;
        }

        let map = &app.per_map.map;
        let req = PathRequest::between_buildings(
            map,
            from,
            self.current_target.unwrap(),
            PathConstraints::Car,
        )?;

        Some(
            map_gui::tools::draw_overlapping_paths(
                app,
                vec![
                    (
                        self.pathfinder_before.pathfind_v2(req.clone(), map)?,
                        *colors::PLAN_ROUTE_BEFORE,
                    ),
                    (
                        self.pathfinder_after.pathfind_v2(req.clone(), map)?,
                        *colors::PLAN_ROUTE_AFTER,
                    ),
                ],
            )
            .unzoomed
            .upload(ctx),
        )
    }
}

impl State<App> for PerResidentImpact {
    fn event(&mut self, ctx: &mut EventCtx, app: &mut App) -> Transition {
        if let PreserveState::PerResidentImpact(_, ref mut x) = self.preserve_state {
            *x = self.current_target;
        } else {
            unreachable!();
        }
        if let Some(t) = self
            .appwide_panel
            .event(ctx, app, &self.preserve_state, help)
        {
            return t;
        }
        if let Some(t) = app.session.layers.event(
            ctx,
            &app.cs,
            Mode::PerResidentImpact,
            Some(&self.bottom_panel),
        ) {
            return t;
        }
        if let Outcome::Clicked(x) = self.bottom_panel.event(ctx) {
            if x == "Back" {
                return Transition::Replace(pages::DesignLTN::new_state(
                    ctx,
                    app,
                    self.neighbourhood.id,
                ));
            } else {
                unreachable!()
            }
        }

        match self.world.event(ctx) {
            WorldOutcome::ClickedObject(Obj::Building(b)) => {
                if self.buildings_inside.contains(&b) {
                    if let Some(target) = self.current_target {
                        pages::RoutePlanner::add_new_trip(
                            app,
                            TripEndpoint::Building(b),
                            TripEndpoint::Building(target),
                        );
                        return Transition::Replace(pages::RoutePlanner::new_state(ctx, app));
                    }
                } else {
                    self.current_target = Some(b);
                    self.update(ctx, app);
                }
            }
            _ => {}
        }

        let key = self.world.get_hovering().map(|x| match x {
            Obj::Building(b) => b,
        });
        if self.current_target.is_some() && key != self.compare_routes.as_ref().map(|(b, _)| *b) {
            if let Some(b) = key {
                self.compare_routes = Some((
                    b,
                    self.compare_routes(ctx, app, b)
                        .unwrap_or_else(|| Drawable::empty(ctx)),
                ));
            } else {
                self.compare_routes = None;
            }
        }

        Transition::Keep
    }

    fn draw(&self, g: &mut GfxCtx, app: &App) {
        g.redraw(&self.fade_irrelevant);
        g.redraw(&self.cell_outline);
        self.appwide_panel.draw(g);
        self.bottom_panel.draw(g);
        app.session.layers.draw(g, app);
        self.labels.draw(g);
        app.per_map.draw_major_road_labels.draw(g);
        app.per_map.draw_all_filters.draw(g);
        self.world.draw(g);
        if let Some((_, ref draw)) = self.compare_routes {
            g.redraw(draw);
        }
        app.per_map.draw_poi_icons.draw(g);
    }

    fn recreate(&mut self, ctx: &mut EventCtx, app: &mut App) -> Box<dyn State<App>> {
        Self::new_state(ctx, app, self.neighbourhood.id, self.current_target)
    }
}

fn help() -> Vec<&'static str> {
    vec!["Use this tool to determine if some residents may have more trouble than others driving somewhere outside the neighbourhood."]
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
enum Obj {
    Building(BuildingID),
}

impl ObjectID for Obj {}
