use abstutil::Tags;
use geom::{ArrowCap, Distance};
use osm2streets::RoadID;
use widgetry::{
    Choice, Color, DrawBaselayer, Drawable, EventCtx, GeomBatch, GfxCtx, HorizontalAlignment, Key,
    Line, Panel, SimpleState, Spinner, State, Text, TextExt, Transition, VerticalAlignment, Widget,
};

use crate::App;

pub struct EditRoad {
    r: RoadID,
    show_direction: Drawable,
}

impl EditRoad {
    pub(crate) fn new_state(ctx: &mut EventCtx, app: &App, r: RoadID) -> Box<dyn State<App>> {
        let road = &app.model.map.streets.roads[&r];

        let mut batch = GeomBatch::new();
        if app.model.intersection_geom {
            batch.push(
                Color::BLACK,
                road.center_line
                    .make_arrow(Distance::meters(1.0), ArrowCap::Triangle),
            );
        } else {
            batch.push(
                Color::BLACK,
                road.reference_line
                    .make_arrow(Distance::meters(1.0), ArrowCap::Triangle),
            );
        }

        let tags = app
            .model
            .map
            .road_to_osm_tags(r)
            .cloned()
            .unwrap_or_else(Tags::empty);

        let mut txt = Text::new();
        for (k, v) in tags.inner() {
            txt.add_line(Line(format!("{} = {}", k, v)).secondary());
        }
        txt.add_line(Line(format!(
            "Length before trimming: {}",
            road.untrimmed_length()
        )));
        if app.model.intersection_geom {
            txt.add_line(Line(format!(
                "Length after trimming: {}",
                road.center_line.length()
            )));
        }
        for (rt, to) in &road.turn_restrictions {
            info!("Simple turn restriction {:?} to {}", rt, to);
        }
        for (via, to) in &road.complicated_turn_restrictions {
            info!("Complicated turn restriction via {} to {}", via, to);
        }
        let info = txt.into_widget(ctx);

        let controls = Widget::col(vec![
            Widget::row(vec![
                "lanes:forward".text_widget(ctx).margin_right(20),
                Spinner::widget(
                    ctx,
                    "lanes:forward",
                    (1, 5),
                    tags.get("lanes:forward")
                        .and_then(|x| x.parse::<usize>().ok())
                        .unwrap_or(1),
                    1,
                ),
            ]),
            Widget::row(vec![
                "lanes:backward".text_widget(ctx).margin_right(20),
                Spinner::widget(
                    ctx,
                    "lanes:backward",
                    (0, 5),
                    tags.get("lanes:backward")
                        .and_then(|x| x.parse::<usize>().ok())
                        .unwrap_or_else(|| if tags.is("oneway", "yes") { 0 } else { 1 }),
                    1,
                ),
            ]),
            Widget::row(vec![
                "sidewalk".text_widget(ctx).margin_right(20),
                Widget::dropdown(
                    ctx,
                    "sidewalk",
                    if tags.is("sidewalk", "both") {
                        "both"
                    } else if tags.is("sidewalk", "none") {
                        "none"
                    } else if tags.is("sidewalk", "left") {
                        "left"
                    } else if tags.is("sidewalk", "right") {
                        "right"
                    } else {
                        "both"
                    }
                    .to_string(),
                    Choice::strings(vec!["both", "none", "left", "right"]),
                ),
            ]),
            Widget::row(vec![
                "parking".text_widget(ctx).margin_right(20),
                Widget::dropdown(
                    ctx,
                    "parking",
                    // TODO Not all possibilities represented here; very simplified.
                    if tags.is("parking:lane:both", "parallel") {
                        "both"
                    } else if tags.is_any("parking:lane:both", vec!["no_parking", "no_stopping"]) {
                        "none"
                    } else if tags.is("parking:lane:left", "parallel") {
                        "left"
                    } else if tags.is("parking:lane:right", "parallel") {
                        "right"
                    } else {
                        "none"
                    }
                    .to_string(),
                    Choice::strings(vec!["both", "none", "left", "right"]),
                ),
            ]),
            Widget::row(vec![
                "Width scale".text_widget(ctx).margin_right(20),
                Spinner::widget(ctx, "width_scale", (0.5, 10.0), 1.0, 0.5),
            ]),
        ]);

        let col = vec![
            Widget::row(vec![
                Line("Editing road").small_heading().into_widget(ctx),
                ctx.style().btn_close_widget(ctx),
            ]),
            Widget::row(vec![info, controls]),
            ctx.style()
                .btn_solid_primary
                .text("Apply")
                .hotkey(Key::Enter)
                .build_def(ctx),
        ];
        let panel = Panel::new_builder(Widget::col(col))
            .aligned(HorizontalAlignment::Left, VerticalAlignment::Top)
            .build(ctx);
        <dyn SimpleState<_>>::new_state(
            panel,
            Box::new(EditRoad {
                r,
                show_direction: ctx.upload(batch),
            }),
        )
    }
}

impl SimpleState<App> for EditRoad {
    fn on_click(
        &mut self,
        ctx: &mut EventCtx,
        app: &mut App,
        x: &str,
        panel: &mut Panel,
    ) -> Transition<App> {
        match x {
            "close" => Transition::Pop,
            "Apply" => {
                app.model.road_deleted(self.r);

                let mut tags = app
                    .model
                    .map
                    .road_to_osm_tags(self.r)
                    .cloned()
                    .unwrap_or_else(Tags::empty);

                tags.remove("lanes");
                tags.remove("oneway");
                let fwd: usize = panel.spinner("lanes:forward");
                let back: usize = panel.spinner("lanes:backward");
                if back == 0 {
                    tags.insert("oneway", "yes");
                    tags.insert("lanes", fwd.to_string());
                } else {
                    tags.insert("lanes", (fwd + back).to_string());
                    tags.insert("lanes:forward", fwd.to_string());
                    tags.insert("lanes:backward", back.to_string());
                }

                tags.insert("sidewalk", panel.dropdown_value::<String, &str>("sidewalk"));

                tags.remove("parking:lane:both");
                tags.remove("parking:lane:left");
                tags.remove("parking:lane:right");
                match panel.dropdown_value::<String, &str>("parking").as_ref() {
                    "both" => {
                        tags.insert("parking:lane:both", "parallel");
                    }
                    "none" => {
                        tags.insert("parking:lane:both", "none");
                    }
                    "left" => {
                        tags.insert("parking:lane:left", "parallel");
                        tags.insert("parking:lane:right", "none");
                    }
                    "right" => {
                        tags.insert("parking:lane:left", "none");
                        tags.insert("parking:lane:right", "parallel");
                    }
                    _ => unreachable!(),
                }

                let road = app.model.map.streets.roads.get_mut(&self.r).unwrap();
                road.lane_specs_ltr =
                    osm2streets::get_lane_specs_ltr(&tags, &app.model.map.streets.config);
                let scale = panel.spinner("width_scale");
                for lane in &mut road.lane_specs_ltr {
                    lane.width *= scale;
                }

                app.model.road_added(ctx, self.r);
                Transition::Pop
            }
            _ => unreachable!(),
        }
    }

    fn panel_changed(
        &mut self,
        ctx: &mut EventCtx,
        app: &mut App,
        panel: &mut Panel,
    ) -> Option<Transition<App>> {
        let scale = panel.spinner("width_scale");
        app.model.road_deleted(self.r);
        let tags = app
            .model
            .map
            .road_to_osm_tags(self.r)
            .cloned()
            .unwrap_or_else(Tags::empty);
        let road = app.model.map.streets.roads.get_mut(&self.r).unwrap();
        road.lane_specs_ltr = osm2streets::get_lane_specs_ltr(&tags, &app.model.map.streets.config);
        for lane in &mut road.lane_specs_ltr {
            lane.width *= scale;
        }
        app.model.road_added(ctx, self.r);
        Some(Transition::Keep)
    }

    fn other_event(&mut self, ctx: &mut EventCtx, _: &mut App) -> Transition<App> {
        ctx.canvas_movement();
        Transition::Keep
    }

    fn draw(&self, g: &mut GfxCtx, _: &App) {
        g.redraw(&self.show_direction);
    }

    fn draw_baselayer(&self) -> DrawBaselayer {
        DrawBaselayer::PreviousState
    }
}
