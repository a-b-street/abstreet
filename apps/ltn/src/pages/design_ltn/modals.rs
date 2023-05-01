use std::collections::BTreeSet;

use geom::{Distance, Polygon};
use map_gui::tools::grey_out_map;
use map_model::{FilterType, RoadFilter, RoadID};
use osm2streets::{Direction, LaneSpec};
use widgetry::{
    Color, ControlState, DrawBaselayer, EventCtx, GeomBatch, GfxCtx, Key, Line, Outcome, Panel,
    RewriteColor, State, Text, Texture, Toggle, Widget,
};

use crate::{redraw_all_filters, render, App, Transition};

pub struct ResolveOneWayAndFilter {
    panel: Panel,
    roads: Vec<(RoadID, Distance)>,
}

impl ResolveOneWayAndFilter {
    pub fn new_state(ctx: &mut EventCtx, roads: Vec<(RoadID, Distance)>) -> Box<dyn State<App>> {
        let mut txt = Text::new();
        txt.add_line(Line("Warning").small_heading());
        txt.add_line("A modal filter cannot be placed on a one-way street.");
        txt.add_line("");
        txt.add_line("You can make the street two-way first, then place a filter.");

        let panel = Panel::new_builder(Widget::col(vec![
            txt.into_widget(ctx),
            Toggle::checkbox(ctx, "Don't show this warning again", None, true),
            ctx.style().btn_solid_primary.text("OK").build_def(ctx),
        ]))
        .build(ctx);

        Box::new(Self { panel, roads })
    }
}

impl State<App> for ResolveOneWayAndFilter {
    fn event(&mut self, ctx: &mut EventCtx, app: &mut App) -> Transition {
        if let Outcome::Clicked(_) = self.panel.event(ctx) {
            // OK is the only choice
            app.session.layers.autofix_one_ways =
                self.panel.is_checked("Don't show this warning again");

            fix_oneway_and_add_filter(ctx, app, &self.roads);

            return Transition::Multi(vec![Transition::Pop, Transition::Recreate]);
        }
        Transition::Keep
    }

    fn draw_baselayer(&self) -> DrawBaselayer {
        DrawBaselayer::PreviousState
    }

    fn draw(&self, g: &mut GfxCtx, app: &App) {
        grey_out_map(g, app);
        self.panel.draw(g);
    }
}

pub fn fix_oneway_and_add_filter(ctx: &mut EventCtx, app: &mut App, roads: &[(RoadID, Distance)]) {
    let driving_side = app.per_map.map.get_config().driving_side;
    let mut edits = app.per_map.map.get_edits().clone();
    for (r, dist) in roads {
        edits
            .commands
            .push(app.per_map.map.edit_road_cmd(*r, |new| {
                LaneSpec::toggle_road_direction(&mut new.lanes_ltr, driving_side);
                // Maybe we just flipped a one-way forwards to a one-way backwards. So one more
                // time to make it two-way
                if LaneSpec::oneway_for_driving(&new.lanes_ltr) == Some(Direction::Back) {
                    LaneSpec::toggle_road_direction(&mut new.lanes_ltr, driving_side);
                }
                new.modal_filter = Some(RoadFilter::new_by_user(*dist, app.session.filter_type));
            }));
    }
    app.apply_edits(edits);
    redraw_all_filters(ctx, app);
}

pub struct ResolveBusGate {
    panel: Panel,
    roads: Vec<(RoadID, Distance)>,
}

impl ResolveBusGate {
    pub fn new_state(
        ctx: &mut EventCtx,
        app: &mut App,
        roads: Vec<(RoadID, Distance)>,
    ) -> Box<dyn State<App>> {
        // TODO This'll mess up the placement, but we don't have easy access to the bottom panel
        // here
        app.session.layers.show_bus_routes(ctx, &app.cs, None);

        let mut txt = Text::new();
        txt.add_line(Line("Warning").small_heading());
        txt.add_line("The following bus routes cross this road. Adding a regular modal filter would block them.");
        txt.add_line("");

        let mut routes = BTreeSet::new();
        for (r, _) in &roads {
            routes.extend(app.per_map.map.get_bus_routes_on_road(*r));
        }
        for route in routes {
            txt.add_line(format!("- {route}"));
        }

        txt.add_line("");
        txt.add_line("You can use a bus gate instead.");

        let panel = Panel::new_builder(Widget::col(vec![
            txt.into_widget(ctx),
            Toggle::checkbox(ctx, "Don't show this warning again", None, true),
            ctx.style().btn_solid_primary.text("OK").build_def(ctx),
        ]))
        .build(ctx);

        Box::new(Self { panel, roads })
    }
}

impl State<App> for ResolveBusGate {
    fn event(&mut self, ctx: &mut EventCtx, app: &mut App) -> Transition {
        if let Outcome::Clicked(_) = self.panel.event(ctx) {
            // OK is the only choice
            app.session.layers.autofix_bus_gates =
                self.panel.is_checked("Don't show this warning again");
            // Force the panel to show the new checkbox state
            app.session.layers.show_bus_routes(ctx, &app.cs, None);

            let mut edits = app.per_map.map.get_edits().clone();
            for (r, dist) in self.roads.drain(..) {
                edits.commands.push(app.per_map.map.edit_road_cmd(r, |new| {
                    new.modal_filter = Some(RoadFilter::new_by_user(dist, FilterType::BusGate));
                }));
            }
            app.apply_edits(edits);
            redraw_all_filters(ctx, app);

            return Transition::Multi(vec![Transition::Pop, Transition::Recreate]);
        }
        Transition::Keep
    }

    fn draw_baselayer(&self) -> DrawBaselayer {
        DrawBaselayer::PreviousState
    }

    fn draw(&self, g: &mut GfxCtx, app: &App) {
        grey_out_map(g, app);
        self.panel.draw(g);
    }
}

pub struct ChangeFilterType {
    panel: Panel,
}

impl ChangeFilterType {
    pub fn new_state(ctx: &mut EventCtx, app: &App) -> Box<dyn State<App>> {
        let filter = |ft: FilterType, hotkey: Key, name: &str| {
            ctx.style()
                .btn_solid_primary
                .icon_text(render::filter_svg_path(ft), name)
                .image_color(
                    RewriteColor::Change(render::filter_hide_color(ft), Color::CLEAR),
                    ControlState::Default,
                )
                .image_color(
                    RewriteColor::Change(render::filter_hide_color(ft), Color::CLEAR),
                    ControlState::Disabled,
                )
                .disabled(app.session.filter_type == ft)
                .hotkey(hotkey)
                .build_def(ctx)
        };

        let panel = Panel::new_builder(Widget::col(vec![
            Widget::row(vec![
                Line("Choose a modal filter to place on streets")
                    .small_heading()
                    .into_widget(ctx),
                ctx.style().btn_close_widget(ctx),
            ]),
            Widget::row(vec![
                Widget::col(vec![
                    filter(
                        FilterType::WalkCycleOnly,
                        Key::Num1,
                        "Walking/cycling only",
                    ),
                    filter(FilterType::NoEntry, Key::Num2, "No entry"),
                    filter(FilterType::BusGate, Key::Num3, "Bus gate"),
                    filter(FilterType::SchoolStreet, Key::Num4, "School street"),
                ]),
                Widget::vertical_separator(ctx),
                Widget::col(vec![
                    GeomBatch::from(vec![
                        (match app.session.filter_type {
                            FilterType::WalkCycleOnly => Texture(1),
                            FilterType::NoEntry => Texture(2),
                            FilterType::BusGate => Texture(3),
                            FilterType::SchoolStreet => Texture(4),
                            // The rectangle size must match the base image, otherwise it'll be
                            // repeated (tiled) or cropped -- not scaled.
                        }, Polygon::rectangle(crate::SPRITE_WIDTH as f64, crate::SPRITE_HEIGHT as f64))
                    ]).into_widget(ctx),
                    // TODO Ambulances, etc
                    Text::from(Line(match app.session.filter_type {
                        FilterType::WalkCycleOnly => "A physical barrier that only allows people walking, cycling, and rolling to pass. Often planters or bollards. Larger vehicles cannot enter.",
                        FilterType::NoEntry => "An alternative sign to indicate vehicles are not allowed to enter the street. Only people walking, cycling, and rolling may pass through.",
                        FilterType::BusGate => "A bus gate sign and traffic cameras are installed to allow buses, pedestrians, and cyclists to pass. There is no physical barrier.",
                        FilterType::SchoolStreet => "A closure during school hours only. The barrier usually allows teachers and staff to access the school.",
                    })).wrap_to_pixels(ctx, crate::SPRITE_WIDTH as f64).into_widget(ctx),
                ]),
            ]),
            ctx.style().btn_solid_primary.text("OK").hotkey(Key::Enter).build_def(ctx).centered_horiz(),
        ]))
        .build(ctx);
        Box::new(Self { panel })
    }
}

impl State<App> for ChangeFilterType {
    fn event(&mut self, ctx: &mut EventCtx, app: &mut App) -> Transition {
        if let Outcome::Clicked(x) = self.panel.event(ctx) {
            return match x.as_ref() {
                "No entry" => {
                    app.session.filter_type = FilterType::NoEntry;
                    Transition::Replace(Self::new_state(ctx, app))
                }
                "Walking/cycling only" => {
                    app.session.filter_type = FilterType::WalkCycleOnly;
                    Transition::Replace(Self::new_state(ctx, app))
                }
                "Bus gate" => {
                    app.session.filter_type = FilterType::BusGate;
                    Transition::Replace(Self::new_state(ctx, app))
                }
                "School street" => {
                    app.session.filter_type = FilterType::SchoolStreet;
                    Transition::Replace(Self::new_state(ctx, app))
                }
                "close" | "OK" => Transition::Multi(vec![Transition::Pop, Transition::Recreate]),
                _ => unreachable!(),
            };
        }

        Transition::Keep
    }

    fn draw_baselayer(&self) -> DrawBaselayer {
        DrawBaselayer::PreviousState
    }

    fn draw(&self, g: &mut GfxCtx, app: &App) {
        grey_out_map(g, app);
        self.panel.draw(g);
    }
}
