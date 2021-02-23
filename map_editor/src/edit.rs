use geom::{ArrowCap, Distance};
use map_model::raw::OriginalRoad;
use widgetry::{
    Choice, Color, DrawBaselayer, Drawable, EventCtx, GeomBatch, GfxCtx, HorizontalAlignment, Key,
    Line, Panel, SimpleState, Spinner, State, StyledButtons, Text, TextExt, Transition,
    VerticalAlignment, Widget,
};

use crate::App;

pub struct EditRoad {
    r: OriginalRoad,
    show_direction: Drawable,
}

impl EditRoad {
    pub(crate) fn new(ctx: &mut EventCtx, app: &App, r: OriginalRoad) -> Box<dyn State<App>> {
        let road = &app.model.map.roads[&r];

        let mut batch = GeomBatch::new();
        if let Some(pl) = app.model.map.trimmed_road_geometry(r) {
            batch.push(
                Color::BLACK,
                pl.make_arrow(Distance::meters(1.0), ArrowCap::Triangle),
            );
        }

        let mut txt = Text::new();
        for (k, v) in road.osm_tags.inner() {
            txt.add(Line(format!("{} = {}", k, v)).secondary());
        }
        let info = txt.draw(ctx);

        let controls = Widget::col(vec![
            Widget::row(vec![
                "lanes:forward".draw_text(ctx).margin_right(20),
                Spinner::new(
                    ctx,
                    (1, 5),
                    road.osm_tags
                        .get("lanes:forward")
                        .and_then(|x| x.parse::<isize>().ok())
                        .unwrap_or(1),
                )
                .named("lanes:forward"),
            ]),
            Widget::row(vec![
                "lanes:backward".draw_text(ctx).margin_right(20),
                Spinner::new(
                    ctx,
                    (0, 5),
                    road.osm_tags
                        .get("lanes:backward")
                        .and_then(|x| x.parse::<isize>().ok())
                        .unwrap_or(1),
                )
                .named("lanes:backward"),
            ]),
            Widget::row(vec![
                "sidewalk".draw_text(ctx).margin_right(20),
                Widget::dropdown(
                    ctx,
                    "sidewalk",
                    if road.osm_tags.is("sidewalk", "both") {
                        "both"
                    } else if road.osm_tags.is("sidewalk", "none") {
                        "none"
                    } else if road.osm_tags.is("sidewalk", "left") {
                        "left"
                    } else if road.osm_tags.is("sidewalk", "right") {
                        "right"
                    } else {
                        "both"
                    }
                    .to_string(),
                    Choice::strings(vec!["both", "none", "left", "right"]),
                ),
            ]),
            Widget::row(vec![
                "parking".draw_text(ctx).margin_right(20),
                Widget::dropdown(
                    ctx,
                    "parking",
                    // TODO Not all possibilities represented here; very simplified.
                    if road.osm_tags.is("parking:lane:both", "parallel") {
                        "both"
                    } else if road
                        .osm_tags
                        .is_any("parking:lane:both", vec!["no_parking", "no_stopping"])
                    {
                        "none"
                    } else if road.osm_tags.is("parking:lane:left", "parallel") {
                        "left"
                    } else if road.osm_tags.is("parking:lane:right", "parallel") {
                        "right"
                    } else {
                        "none"
                    }
                    .to_string(),
                    Choice::strings(vec!["both", "none", "left", "right"]),
                ),
            ]),
        ]);

        let col = vec![
            Widget::row(vec![
                Line("Editing road").small_heading().draw(ctx),
                ctx.style().btn_close_widget(ctx),
            ]),
            Widget::row(vec![info, controls]),
            ctx.style()
                .btn_solid_text("Apply")
                .hotkey(Key::Enter)
                .build_def(ctx),
        ];
        let panel = Panel::new(Widget::col(col))
            .aligned(HorizontalAlignment::Left, VerticalAlignment::Top)
            .build(ctx);
        SimpleState::new(
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
        panel: &Panel,
    ) -> Transition<App> {
        match x {
            "close" => Transition::Pop,
            "Apply" => {
                app.model.road_deleted(self.r);

                let road = app.model.map.roads.get_mut(&self.r).unwrap();

                road.osm_tags.remove("lanes");
                road.osm_tags.remove("oneway");
                let fwd = panel.spinner("lanes:forward") as usize;
                let back = panel.spinner("lanes:backward") as usize;
                if back == 0 {
                    road.osm_tags.insert("oneway", "yes");
                    road.osm_tags.insert("lanes", fwd.to_string());
                } else {
                    road.osm_tags.insert("lanes", (fwd + back).to_string());
                    road.osm_tags.insert("lanes:forward", fwd.to_string());
                    road.osm_tags.insert("lanes:backward", back.to_string());
                }

                road.osm_tags
                    .insert("sidewalk", panel.dropdown_value::<String, &str>("sidewalk"));

                road.osm_tags.remove("parking:lane:both");
                road.osm_tags.remove("parking:lane:left");
                road.osm_tags.remove("parking:lane:right");
                match panel.dropdown_value::<String, &str>("parking").as_ref() {
                    "both" => {
                        road.osm_tags.insert("parking:lane:both", "parallel");
                    }
                    "none" => {
                        road.osm_tags.insert("parking:lane:both", "none");
                    }
                    "left" => {
                        road.osm_tags.insert("parking:lane:left", "parallel");
                        road.osm_tags.insert("parking:lane:right", "none");
                    }
                    "right" => {
                        road.osm_tags.insert("parking:lane:left", "none");
                        road.osm_tags.insert("parking:lane:right", "parallel");
                    }
                    _ => unreachable!(),
                }

                app.model.road_added(ctx, self.r);
                Transition::Pop
            }
            _ => unreachable!(),
        }
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
