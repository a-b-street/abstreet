use std::collections::BTreeSet;

use maplit::btreeset;

use geom::{Distance, Duration};
use map_model::IntersectionID;
use sim::Scenario;
use widgetry::{
    Btn, Color, Drawable, EventCtx, GfxCtx, HorizontalAlignment, Key, Line, Panel, RewriteColor,
    SimpleState, Spinner, State, Text, TextExt, VerticalAlignment, Widget,
};

use crate::app::{App, Transition};
use crate::common::CommonState;
use crate::edit::traffic_signals::fade_irrelevant;

pub struct ShowAbsolute {
    members: BTreeSet<IntersectionID>,
    labels: Drawable,
}

impl ShowAbsolute {
    pub fn new(
        ctx: &mut EventCtx,
        app: &App,
        members: BTreeSet<IntersectionID>,
    ) -> Box<dyn State<App>> {
        let mut batch = fade_irrelevant(app, &members);
        for i in &members {
            batch.append(
                Text::from(Line(
                    app.primary
                        .map
                        .get_traffic_signal(*i)
                        .offset
                        .to_string(&app.opts.units),
                ))
                .bg(Color::PURPLE)
                .render_autocropped(ctx)
                .color(RewriteColor::ChangeAlpha(0.8))
                .scale(0.3)
                .centered_on(app.primary.map.get_i(*i).polygon.center()),
            );
        }

        let panel = Panel::new(Widget::col(vec![
            Widget::row(vec![
                Line(format!("Tuning offset for {} signals", members.len()))
                    .small_heading()
                    .draw(ctx),
                Btn::close(ctx),
            ]),
            "Select an intersection as the base".draw_text(ctx),
        ]))
        .aligned(HorizontalAlignment::Center, VerticalAlignment::Top)
        .build(ctx);
        SimpleState::new(
            panel,
            Box::new(ShowAbsolute {
                members,
                labels: ctx.upload(batch),
            }),
        )
    }
}

impl SimpleState<App> for ShowAbsolute {
    fn on_click(&mut self, _: &mut EventCtx, _: &mut App, x: &str, _: &Panel) -> Transition {
        match x {
            "close" => {
                // TODO Bit confusing UX, because all the offset changes won't show up in the
                // undo stack. Could maybe do ReplaceWithData.
                Transition::Pop
            }
            _ => unreachable!(),
        }
    }

    fn on_mouseover(&mut self, ctx: &mut EventCtx, app: &mut App) {
        app.primary.current_selection = app
            .mouseover_unzoomed_intersections(ctx)
            .filter(|id| self.members.contains(&id.as_intersection()));
    }

    fn other_event(&mut self, ctx: &mut EventCtx, app: &mut App) -> Transition {
        ctx.canvas_movement();
        if let Some(i) = app.click_on_intersection(ctx, "select base intersection") {
            return Transition::Replace(ShowRelative::new(ctx, app, i, self.members.clone()));
        }

        Transition::Keep
    }

    fn draw(&self, g: &mut GfxCtx, app: &App) {
        CommonState::draw_osd(g, app);

        g.redraw(&self.labels);
    }
}

struct ShowRelative {
    base: IntersectionID,
    members: BTreeSet<IntersectionID>,
    labels: Drawable,
}

impl ShowRelative {
    pub fn new(
        ctx: &mut EventCtx,
        app: &App,
        base: IntersectionID,
        members: BTreeSet<IntersectionID>,
    ) -> Box<dyn State<App>> {
        let base_offset = app.primary.map.get_traffic_signal(base).offset;
        let mut batch = fade_irrelevant(app, &members);
        for i in &members {
            if *i == base {
                batch.push(
                    Color::BLUE.alpha(0.8),
                    app.primary.map.get_i(*i).polygon.clone(),
                );
            } else {
                let offset = app.primary.map.get_traffic_signal(*i).offset - base_offset;
                batch.append(
                    Text::from(Line(offset.to_string(&app.opts.units)))
                        .bg(Color::PURPLE)
                        .render_autocropped(ctx)
                        .color(RewriteColor::ChangeAlpha(0.8))
                        .scale(0.3)
                        .centered_on(app.primary.map.get_i(*i).polygon.center()),
                );
            }
        }

        let panel = Panel::new(Widget::col(vec![
            Widget::row(vec![
                Line(format!("Tuning offset for {} signals", members.len()))
                    .small_heading()
                    .draw(ctx),
                Btn::close(ctx),
            ]),
            "Select a second intersection to tune offset between the two".draw_text(ctx),
        ]))
        .aligned(HorizontalAlignment::Center, VerticalAlignment::Top)
        .build(ctx);
        SimpleState::new(
            panel,
            Box::new(ShowRelative {
                base,
                members,
                labels: ctx.upload(batch),
            }),
        )
    }
}

impl SimpleState<App> for ShowRelative {
    fn on_click(&mut self, ctx: &mut EventCtx, app: &mut App, x: &str, _: &Panel) -> Transition {
        match x {
            "close" => Transition::Replace(ShowAbsolute::new(ctx, app, self.members.clone())),
            _ => unreachable!(),
        }
    }

    fn on_mouseover(&mut self, ctx: &mut EventCtx, app: &mut App) {
        app.primary.current_selection = app.mouseover_unzoomed_intersections(ctx).filter(|id| {
            let i = id.as_intersection();
            self.members.contains(&i) && i != self.base
        });
    }

    fn other_event(&mut self, ctx: &mut EventCtx, app: &mut App) -> Transition {
        ctx.canvas_movement();
        if let Some(i) = app.click_on_intersection(ctx, "select second intersection") {
            return Transition::Push(TuneRelative::new(
                ctx,
                app,
                self.base,
                i,
                self.members.clone(),
            ));
        }

        Transition::Keep
    }

    fn draw(&self, g: &mut GfxCtx, app: &App) {
        CommonState::draw_osd(g, app);

        g.redraw(&self.labels);
    }
}

struct TuneRelative {
    i1: IntersectionID,
    i2: IntersectionID,
    members: BTreeSet<IntersectionID>,
    labels: Drawable,
}

impl TuneRelative {
    pub fn new(
        ctx: &mut EventCtx,
        app: &App,
        i1: IntersectionID,
        i2: IntersectionID,
        members: BTreeSet<IntersectionID>,
    ) -> Box<dyn State<App>> {
        let mut batch = fade_irrelevant(app, &btreeset! {i1, i2});
        let map = &app.primary.map;
        // TODO Colors aren't clear. Show directionality.
        batch.push(Color::BLUE.alpha(0.8), map.get_i(i1).polygon.clone());
        batch.push(Color::RED.alpha(0.8), map.get_i(i2).polygon.clone());
        let path = map.simple_path_btwn(i1, i2).unwrap_or_else(Vec::new);
        let mut dist_btwn = Distance::ZERO;
        let mut car_dt = Duration::ZERO;
        for r in path {
            let r = map.get_r(r);
            // TODO Glue polylines together and do dashed_lines
            batch.push(app.cs.route, r.get_thick_polygon(map));
            dist_btwn += r.center_pts.length();
            car_dt += r.center_pts.length() / r.speed_limit;
        }

        let offset1 = map.get_traffic_signal(i1).offset;
        let offset2 = map.get_traffic_signal(i2).offset;
        let panel = Panel::new(Widget::col(vec![
            Widget::row(vec![
                Line(format!("Tuning offset between {} and {}", i1, i2))
                    .small_heading()
                    .draw(ctx),
                Btn::close(ctx),
            ]),
            Text::from_multiline(vec![
                Line(format!("Distance: {}", dist_btwn)),
                Line(format!(
                    "  about {} for a car if there's no congestion",
                    car_dt
                )),
                Line(format!(
                    "  about {} for a bike",
                    dist_btwn / Scenario::max_bike_speed()
                )),
                Line(format!(
                    "  about {} for a pedestrian",
                    dist_btwn / Scenario::max_ped_speed()
                )),
            ])
            .draw(ctx),
            Widget::row(vec![
                "Offset (seconds):".draw_text(ctx),
                Spinner::new(ctx, (0, 90), (offset2 - offset1).inner_seconds() as isize)
                    .named("offset"),
            ]),
            Btn::text_bg2("Update offset").build_def(ctx, Key::Enter),
        ]))
        .build(ctx);
        SimpleState::new(
            panel,
            Box::new(TuneRelative {
                i1,
                i2,
                members,
                labels: ctx.upload(batch),
            }),
        )
    }
}

impl SimpleState<App> for TuneRelative {
    fn on_click(
        &mut self,
        ctx: &mut EventCtx,
        app: &mut App,
        x: &str,
        panel: &Panel,
    ) -> Transition {
        match x {
            "close" => Transition::Pop,
            "Update offset" => {
                let mut ts = app.primary.map.get_traffic_signal(self.i2).clone();
                let relative = Duration::seconds(panel.spinner("offset") as f64);
                let offset1 = app.primary.map.get_traffic_signal(self.i1).offset;
                ts.offset = offset1 + relative;
                app.primary.map.incremental_edit_traffic_signal(ts);
                Transition::Multi(vec![
                    Transition::Pop,
                    Transition::Replace(ShowRelative::new(ctx, app, self.i1, self.members.clone())),
                ])
            }
            _ => unreachable!(),
        }
    }

    fn other_event(&mut self, ctx: &mut EventCtx, _: &mut App) -> Transition {
        ctx.canvas_movement();
        Transition::Keep
    }

    fn draw(&self, g: &mut GfxCtx, _: &App) {
        g.redraw(&self.labels);
    }
}
