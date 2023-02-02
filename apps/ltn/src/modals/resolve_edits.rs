struct ResolveOneWayAndFilter {
    panel: Panel,
    roads: Vec<(RoadID, Distance)>,
}

impl ResolveOneWayAndFilter {
    fn new_state(ctx: &mut EventCtx, roads: Vec<(RoadID, Distance)>) -> Box<dyn State<App>> {
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

fn fix_oneway_and_add_filter(ctx: &mut EventCtx, app: &mut App, roads: &[(RoadID, Distance)]) {
    let driving_side = app.per_map.map.get_config().driving_side;
    let mut edits = app.per_map.map.get_edits().clone();
    for (r, _) in roads {
        edits
            .commands
            .push(app.per_map.map.edit_road_cmd(*r, |new| {
                LaneSpec::toggle_road_direction(&mut new.lanes_ltr, driving_side);
                // Maybe we just flipped a one-way forwards to a one-way backwards. So one more
                // time to make it two-way
                if LaneSpec::oneway_for_driving(&new.lanes_ltr) == Some(Direction::Back) {
                    LaneSpec::toggle_road_direction(&mut new.lanes_ltr, driving_side);
                }
            }));
    }
    ctx.loading_screen("apply edits", |_, timer| {
        app.per_map.map.must_apply_edits(edits, timer);
    });

    app.per_map.proposals.before_edit();

    for (r, dist) in roads {
        let r = *r;
        let road = app.per_map.map.get_r(r);
        let r_edit = app.per_map.map.get_r_edit(r);
        if r_edit == EditRoad::get_orig_from_osm(road, app.per_map.map.get_config()) {
            mut_edits!(app).one_ways.remove(&r);
        } else {
            mut_edits!(app).one_ways.insert(r, r_edit);
        }

        mut_edits!(app)
            .roads
            .insert(r, RoadFilter::new_by_user(*dist, app.session.filter_type));
    }

    redraw_all_filters(ctx, app);
}

struct ResolveBusGate {
    panel: Panel,
    roads: Vec<(RoadID, Distance)>,
}

impl ResolveBusGate {
    fn new_state(
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

            app.per_map.proposals.before_edit();
            for (r, dist) in self.roads.drain(..) {
                mut_edits!(app)
                    .roads
                    .insert(r, RoadFilter::new_by_user(dist, FilterType::BusGate));
            }
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
