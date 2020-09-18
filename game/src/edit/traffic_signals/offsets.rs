use crate::app::{App, ShowEverything};
use crate::common::CommonState;
use crate::edit::traffic_signals::fade_irrelevant;
use crate::game::{State, Transition};
use crate::helpers::ID;
use map_model::IntersectionID;
use sim::DontDrawAgents;
use std::collections::BTreeSet;
use widgetry::{
    Btn, Color, Drawable, EventCtx, GfxCtx, HorizontalAlignment, Key, Line, Outcome, Panel,
    RewriteColor, Text, TextExt, VerticalAlignment, Widget,
};

pub struct ShowAbsolute {
    members: BTreeSet<IntersectionID>,
    panel: Panel,
    labels: Drawable,
}

impl ShowAbsolute {
    pub fn new(ctx: &mut EventCtx, app: &App, members: BTreeSet<IntersectionID>) -> Box<dyn State> {
        let mut batch = fade_irrelevant(app, &members);
        for i in &members {
            batch.append(
                Text::from(Line(
                    app.primary.map.get_traffic_signal(*i).offset.to_string(),
                ))
                .bg(Color::PURPLE)
                .render_to_batch(ctx.prerender)
                .color(RewriteColor::ChangeAlpha(0.8))
                .scale(0.3)
                .centered_on(app.primary.map.get_i(*i).polygon.center()),
            );
        }

        Box::new(ShowAbsolute {
            panel: Panel::new(Widget::col(vec![
                Widget::row(vec![
                    Line(format!("Tuning offset for {} signals", members.len()))
                        .small_heading()
                        .draw(ctx),
                    Btn::plaintext("X")
                        .build(ctx, "close", Key::Escape)
                        .align_right(),
                ]),
                "Select an intersection as the base".draw_text(ctx),
            ]))
            .aligned(HorizontalAlignment::Center, VerticalAlignment::Top)
            .build(ctx),
            members,
            labels: ctx.upload(batch),
        })
    }
}

impl State for ShowAbsolute {
    fn event(&mut self, ctx: &mut EventCtx, app: &mut App) -> Transition {
        ctx.canvas_movement();
        if ctx.redo_mouseover() {
            app.primary.current_selection = app.calculate_current_selection(
                ctx,
                &DontDrawAgents {},
                &ShowEverything::new(),
                false,
                true,
                false,
            );
        }
        if let Some(ID::Intersection(i)) = app.primary.current_selection {
            if self.members.contains(&i) {
                if app.per_obj.left_click(ctx, "select base intersection") {
                    return Transition::Push(ShowRelative::new(ctx, app, i, self.members.clone()));
                }
            } else {
                app.primary.current_selection = None;
            }
        } else {
            app.primary.current_selection = None;
        }

        match self.panel.event(ctx) {
            Outcome::Clicked(x) => match x.as_ref() {
                "close" => {
                    return Transition::Pop;
                }
                _ => unreachable!(),
            },
            _ => {}
        }

        Transition::Keep
    }

    fn draw(&self, g: &mut GfxCtx, app: &App) {
        self.panel.draw(g);
        CommonState::draw_osd(g, app);

        g.redraw(&self.labels);
    }
}

struct ShowRelative {
    base: IntersectionID,
    members: BTreeSet<IntersectionID>,
    panel: Panel,
    labels: Drawable,
}

impl ShowRelative {
    pub fn new(
        ctx: &mut EventCtx,
        app: &App,
        base: IntersectionID,
        members: BTreeSet<IntersectionID>,
    ) -> Box<dyn State> {
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
                    Text::from(Line(offset.to_string()))
                        .bg(Color::PURPLE)
                        .render_to_batch(ctx.prerender)
                        .color(RewriteColor::ChangeAlpha(0.8))
                        .scale(0.3)
                        .centered_on(app.primary.map.get_i(*i).polygon.center()),
                );
            }
        }

        Box::new(ShowRelative {
            panel: Panel::new(Widget::col(vec![
                Widget::row(vec![
                    Line(format!("Tuning offset for {} signals", members.len()))
                        .small_heading()
                        .draw(ctx),
                    Btn::plaintext("X")
                        .build(ctx, "close", Key::Escape)
                        .align_right(),
                ]),
                "Select a second intersection to tune offset between the two".draw_text(ctx),
            ]))
            .aligned(HorizontalAlignment::Center, VerticalAlignment::Top)
            .build(ctx),
            base,
            members,
            labels: ctx.upload(batch),
        })
    }
}

impl State for ShowRelative {
    fn event(&mut self, ctx: &mut EventCtx, app: &mut App) -> Transition {
        ctx.canvas_movement();
        if ctx.redo_mouseover() {
            app.primary.current_selection = app.calculate_current_selection(
                ctx,
                &DontDrawAgents {},
                &ShowEverything::new(),
                false,
                true,
                false,
            );
        }
        if let Some(ID::Intersection(i)) = app.primary.current_selection {
            if self.members.contains(&i) && i != self.base {
                if app.per_obj.left_click(ctx, "select second intersection") {
                    // TODO
                }
            } else {
                app.primary.current_selection = None;
            }
        } else {
            app.primary.current_selection = None;
        }

        match self.panel.event(ctx) {
            Outcome::Clicked(x) => match x.as_ref() {
                "close" => {
                    return Transition::Pop;
                }
                _ => unreachable!(),
            },
            _ => {}
        }

        Transition::Keep
    }

    fn draw(&self, g: &mut GfxCtx, app: &App) {
        self.panel.draw(g);
        CommonState::draw_osd(g, app);

        g.redraw(&self.labels);
    }
}
