// To run:
// > cargo run --example demo --features glium-backend
//
// Try the web version, but there's no text rendering yet:
// > cargo web start --target wasm32-unknown-unknown --features wasm-backend --example demo

use ezgui::{
    hotkey, lctrl, Button, Color, Composite, Drawable, EventCtx, EventLoopMode, GeomBatch, GfxCtx,
    HorizontalAlignment, Key, Line, ManagedWidget, Outcome, Plot, PlotOptions, Series, Text,
    VerticalAlignment, GUI,
};
use geom::{Angle, Duration, Polygon, Pt2D, Time};

struct App {
    top_center: Composite,
    draw: Drawable,
    side_panel: Option<(Duration, Composite)>,

    elapsed: Duration,
}

impl App {
    fn new(ctx: &mut EventCtx) -> App {
        let mut batch = GeomBatch::new();
        batch.push(
            Color::hex("#4E30A6"),
            Polygon::rounded_rectangle(5000.0, 5000.0, 25.0),
        );
        batch.add_svg(
            &ctx.prerender,
            "../data/system/assets/pregame/logo.svg",
            Pt2D::new(300.0, 300.0),
            1.0,
            Angle::ZERO,
        );
        batch.add_transformed(
            Text::from(
                Line("Awesome vector text thanks to usvg and lyon").fg(Color::hex("#DF8C3D")),
            )
            .render_to_batch(&ctx.prerender),
            Pt2D::new(600.0, 500.0),
            2.0,
            Angle::new_degs(30.0),
        );

        ctx.canvas.map_dims = (5000.0, 5000.0);

        App {
            top_center: Composite::new(
                ManagedWidget::col(vec![
                    ManagedWidget::draw_text(ctx, {
                        let mut txt = Text::from(Line("ezgui demo").roboto_bold());
                        txt.add(Line(
                            "Click and drag to pan, use touchpad or scroll wheel to zoom",
                        ));
                        txt
                    }),
                    ManagedWidget::row(vec![
                        ManagedWidget::custom_checkbox(
                            false,
                            Button::text_bg(
                                Text::from(Line("Pause")),
                                Color::BLUE,
                                Color::ORANGE,
                                hotkey(Key::Space),
                                "pause the stopwatch",
                                ctx,
                            ),
                            Button::text_bg(
                                Text::from(Line("Resume")),
                                Color::BLUE,
                                Color::ORANGE,
                                hotkey(Key::Space),
                                "resume the stopwatch",
                                ctx,
                            ),
                        )
                        .named("paused")
                        .margin(5),
                        ManagedWidget::btn(Button::text_no_bg(
                            Text::from(Line("Reset")),
                            Text::from(Line("Reset").fg(Color::ORANGE)),
                            None,
                            "reset the stopwatch",
                            true,
                            ctx,
                        ))
                        .outline(3.0, Color::WHITE)
                        .margin(5),
                        ManagedWidget::checkbox(ctx, "Draw scrollable canvas", None, true)
                            .margin(5),
                        ManagedWidget::checkbox(ctx, "Show timeseries", lctrl(Key::T), false)
                            .margin(5),
                    ])
                    .evenly_spaced(),
                    ManagedWidget::draw_text(ctx, Text::from(Line("Stopwatch: ...")))
                        .named("stopwatch"),
                ])
                .bg(Color::grey(0.4)),
            )
            .aligned(HorizontalAlignment::Center, VerticalAlignment::Top)
            .build(ctx),
            draw: batch.upload(ctx),
            side_panel: None,

            elapsed: Duration::ZERO,
        }
    }

    fn make_sidepanel(&self, ctx: &mut EventCtx) -> Composite {
        let mut col1 = vec![ManagedWidget::draw_text(
            ctx,
            Text::from(Line("Time").roboto_bold()),
        )];
        let mut col2 = vec![ManagedWidget::draw_text(
            ctx,
            Text::from(Line("Linear").roboto_bold()),
        )];
        let mut col3 = vec![ManagedWidget::draw_text(
            ctx,
            Text::from(Line("Quadratic").roboto_bold()),
        )];
        for s in 0..(self.elapsed.inner_seconds() as usize) {
            col1.push(ManagedWidget::draw_text(
                ctx,
                Text::from(Line(Duration::seconds(s as f64).to_string())),
            ));
            col2.push(ManagedWidget::draw_text(
                ctx,
                Text::from(Line(s.to_string())),
            ));
            col3.push(ManagedWidget::draw_text(
                ctx,
                Text::from(Line(s.pow(2).to_string())),
            ));
        }

        let mut c = Composite::new(
            ManagedWidget::col(vec![
                ManagedWidget::row(vec![ManagedWidget::draw_text(ctx, {
                    let mut txt = Text::from(
                        Line("Here's a bunch of text to force some scrolling.").roboto_bold(),
                    );
                    txt.add(
                        Line(
                            "Bug: scrolling by clicking and dragging doesn't work while the \
                             stopwatch is running.",
                        )
                        .fg(Color::RED),
                    );
                    txt
                })]),
                ManagedWidget::row(vec![
                    ManagedWidget::col(col1)
                        .outline(3.0, Color::BLACK)
                        .margin(5),
                    ManagedWidget::col(col2)
                        .outline(3.0, Color::BLACK)
                        .margin(5),
                    ManagedWidget::col(col3)
                        .outline(3.0, Color::BLACK)
                        .margin(5),
                ]),
                Plot::new_usize(
                    ctx,
                    vec![
                        Series {
                            label: "Linear".to_string(),
                            color: Color::GREEN,
                            pts: (0..(self.elapsed.inner_seconds() as usize))
                                .map(|s| (Time::START_OF_DAY + Duration::seconds(s as f64), s))
                                .collect(),
                        },
                        Series {
                            label: "Quadratic".to_string(),
                            color: Color::BLUE,
                            pts: (0..(self.elapsed.inner_seconds() as usize))
                                .map(|s| {
                                    (Time::START_OF_DAY + Duration::seconds(s as f64), s.pow(2))
                                })
                                .collect(),
                        },
                    ],
                    PlotOptions {
                        max_x: Some(Time::START_OF_DAY + self.elapsed),
                    },
                ),
            ])
            .bg(Color::grey(0.4)),
        )
        .max_size_percent(30, 40)
        .aligned(HorizontalAlignment::Percent(0.6), VerticalAlignment::Center)
        .build(ctx);

        if let Some((_, ref old)) = self.side_panel {
            c.restore_scroll(ctx, old.preserve_scroll());
        }
        c
    }
}

impl GUI for App {
    fn event(&mut self, ctx: &mut EventCtx) -> EventLoopMode {
        ctx.canvas_movement();

        match self.top_center.event(ctx) {
            Some(Outcome::Clicked(x)) => match x.as_ref() {
                "pause the stopwatch" | "resume the stopwatch" => {}
                "reset the stopwatch" => {
                    self.elapsed = Duration::ZERO;
                    self.top_center.replace(
                        ctx,
                        "stopwatch",
                        ManagedWidget::draw_text(
                            ctx,
                            Text::from(Line(format!("Stopwatch: {}", self.elapsed))),
                        )
                        .named("stopwatch"),
                    );
                }
                _ => unreachable!(),
            },
            None => {}
        }

        if let Some(dt) = ctx.input.nonblocking_is_update_event() {
            ctx.input.use_update_event();
            self.elapsed += dt;
            self.top_center.replace(
                ctx,
                "stopwatch",
                ManagedWidget::draw_text(
                    ctx,
                    Text::from(Line(format!("Stopwatch: {}", self.elapsed))),
                )
                .named("stopwatch"),
            );
        }

        if self.top_center.is_checked("Show timeseries") {
            if self
                .side_panel
                .as_ref()
                .map(|(dt, _)| *dt != self.elapsed)
                .unwrap_or(true)
            {
                self.side_panel = Some((self.elapsed, self.make_sidepanel(ctx)));
            }
        } else {
            self.side_panel = None;
        }

        if let Some((_, ref mut p)) = self.side_panel {
            match p.event(ctx) {
                // No buttons in there
                Some(Outcome::Clicked(_)) => unreachable!(),
                None => {}
            }
        }

        if self.top_center.is_checked("paused") {
            EventLoopMode::InputOnly
        } else {
            EventLoopMode::Animation
        }
    }

    fn draw(&self, g: &mut GfxCtx) {
        g.clear(Color::BLACK);

        if self.top_center.is_checked("Draw scrollable canvas") {
            g.redraw(&self.draw);
        }

        self.top_center.draw(g);

        if let Some((_, ref p)) = self.side_panel {
            p.draw(g);
        }
    }
}

fn main() {
    ezgui::run(
        ezgui::Settings::new("ezgui demo", "../data/system/fonts"),
        |ctx| App::new(ctx),
    );
}
