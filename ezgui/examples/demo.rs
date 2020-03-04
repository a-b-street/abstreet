use ezgui::{
    hotkey, Button, Color, Composite, Drawable, EventCtx, EventLoopMode, GeomBatch, GfxCtx,
    HorizontalAlignment, Key, Line, ManagedWidget, Outcome, Text, VerticalAlignment, GUI,
};
use geom::{Angle, Duration, Polygon, Pt2D};

// TODO Add text to the logo (showing zoom)
// TODO Some kind of plot?!
// TODO Some popup dialogs with form entry, even some scrolling
// TODO Loading screen with timer?

struct App {
    top_center: Composite,
    draw: Drawable,

    paused: bool,
    elapsed: Duration,
}

impl App {
    fn new(ctx: &mut EventCtx) -> App {
        let mut batch = GeomBatch::new();
        batch.push(Color::RED, Polygon::rounded_rectangle(5000.0, 5000.0, 25.0));
        batch.add_svg(
            &ctx.prerender,
            "../data/system/assets/pregame/logo.svg",
            Pt2D::new(300.0, 300.0),
            1.0,
            Angle::ZERO,
        );
        batch.add_transformed(
            Text::from(Line("Awesome vector text thanks to usvg and lyon"))
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
                        ManagedWidget::btn(Button::text_bg(
                            Text::from(Line("Pause")),
                            Color::BLUE,
                            Color::ORANGE,
                            hotkey(Key::Space),
                            "pause the stopwatch",
                            ctx,
                        )),
                        ManagedWidget::btn(Button::text_no_bg(
                            Text::from(Line("Reset")),
                            Text::from(Line("Reset").fg(Color::ORANGE)),
                            None,
                            "reset the stopwatch",
                            true,
                            ctx,
                        ))
                        .outline(3.0, Color::WHITE),
                        ManagedWidget::checkbox(ctx, "Draw logo", None, true),
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

            paused: false,
            elapsed: Duration::ZERO,
        }
    }
}

impl GUI for App {
    fn event(&mut self, ctx: &mut EventCtx) -> EventLoopMode {
        ctx.canvas_movement();

        match self.top_center.event(ctx) {
            Some(Outcome::Clicked(x)) => match x.as_ref() {
                "pause the stopwatch" => {
                    self.paused = true;
                    self.top_center.replace(
                        ctx,
                        "pause the stopwatch",
                        ManagedWidget::btn(Button::text_bg(
                            Text::from(Line("Resume")),
                            Color::BLUE,
                            Color::ORANGE,
                            hotkey(Key::Space),
                            "resume the stopwatch",
                            ctx,
                        )),
                    );
                }
                "resume the stopwatch" => {
                    self.paused = false;
                    self.top_center.replace(
                        ctx,
                        "resume the stopwatch",
                        ManagedWidget::btn(Button::text_bg(
                            Text::from(Line("Pause")),
                            Color::BLUE,
                            Color::ORANGE,
                            hotkey(Key::Space),
                            "pause the stopwatch",
                            ctx,
                        )),
                    );
                }
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

        if self.paused {
            EventLoopMode::InputOnly
        } else {
            EventLoopMode::Animation
        }
    }

    fn draw(&self, g: &mut GfxCtx) {
        g.clear(Color::BLACK);

        if self.top_center.is_checked("Draw logo") {
            g.redraw(&self.draw);
        }

        self.top_center.draw(g);
    }
}

fn main() {
    ezgui::run(
        ezgui::Settings::new("ezgui demo", "../data/system/fonts"),
        |ctx| App::new(ctx),
    );
}
