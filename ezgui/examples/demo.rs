// You have to run from the ezgui crate (abstreet/ezgui), due to relative paths to fonts and
// images.
//
// To run:
// > cargo run --example demo --features glium-backend
//
// Try the web version, but there's no text rendering yet:
// > cargo web start --target wasm32-unknown-unknown --features wasm-backend --example demo

use ezgui::{
    hotkey, lctrl, Button, Color, Composite, Drawable, EventCtx, EventLoopMode, GeomBatch, GfxCtx,
    HorizontalAlignment, Key, Line, ManagedWidget, Outcome, Plot, PlotOptions, Series, Text,
    TextExt, VerticalAlignment, GUI,
};
use geom::{Angle, Duration, Polygon, Pt2D, Time};

fn main() {
    // Control flow surrendered here. App implements State, which has an event handler and a draw
    // callback.
    ezgui::run(
        ezgui::Settings::new("ezgui demo", "../data/system/fonts"),
        |ctx| App::new(ctx),
    );
}

struct App {
    controls: Composite,
    timeseries_panel: Option<(Duration, Composite)>,
    scrollable_canvas: Drawable,
    elapsed: Duration,
}

impl App {
    fn new(ctx: &mut EventCtx) -> App {
        App {
            controls: make_controls(ctx),
            timeseries_panel: None,
            scrollable_canvas: setup_scrollable_canvas(ctx),
            elapsed: Duration::ZERO,
        }
    }

    fn make_timeseries_panel(&self, ctx: &mut EventCtx) -> Composite {
        // Make a table with 3 columns.
        let mut col1 = vec![Line("Time").roboto_bold().draw(ctx)];
        let mut col2 = vec![Line("Linear").roboto_bold().draw(ctx)];
        let mut col3 = vec![Line("Quadratic").roboto_bold().draw(ctx)];
        for s in 0..(self.elapsed.inner_seconds() as usize) {
            col1.push(Duration::seconds(s as f64).to_string().draw_text(ctx));
            col2.push(s.to_string().draw_text(ctx));
            col3.push(s.pow(2).to_string().draw_text(ctx));
        }

        let mut c = Composite::new(
            ManagedWidget::col(vec![
                ManagedWidget::row(vec![{
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
                    txt.draw(ctx)
                }]),
                ManagedWidget::row(vec![
                    // Examples of styling widgets
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
                            // These points are (x axis = Time, y axis = usize)
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
                        // Without this, the plot doesn't stretch to cover times in between whole
                        // seconds.
                        max_x: Some(Time::START_OF_DAY + self.elapsed),
                    },
                ),
            ])
            .bg(Color::grey(0.4)),
        )
        // Don't let the panel exceed this percentage of the window. Scrollbars appear
        // automatically if needed.
        .max_size_percent(30, 40)
        // We take up 30% width, and we want to leave 10% window width as buffer.
        .aligned(HorizontalAlignment::Percent(0.6), VerticalAlignment::Center)
        .build(ctx);

        // Since we're creating an entirely new panel when the time changes, it's nice to keep the
        // scroll the same. If the new panel got shorter and the old scroll was too long, this
        // clamps reasonably.
        if let Some((_, ref old)) = self.timeseries_panel {
            c.restore_scroll(ctx, old.preserve_scroll());
        }
        c
    }
}

impl GUI for App {
    fn event(&mut self, ctx: &mut EventCtx) -> EventLoopMode {
        // Allow panning and zooming to work.
        ctx.canvas_movement();

        // This dispatches event handling to all of the widgets inside.
        match self.controls.event(ctx) {
            Some(Outcome::Clicked(x)) => match x.as_ref() {
                // These outcomes should probably be a custom enum per Composite, to be more
                // typesafe.
                "reset the stopwatch" => {
                    self.elapsed = Duration::ZERO;
                    // We can replace any named widget with another one. Layout gets recalculated.
                    self.controls.replace(
                        ctx,
                        "stopwatch",
                        format!("Stopwatch: {}", self.elapsed)
                            .draw_text(ctx)
                            .named("stopwatch"),
                    );
                }
                _ => unreachable!(),
            },
            None => {}
        }

        // An update event means that no keyboard/mouse input happened, but time has passed.
        // (Ignore the "nonblocking"; this API is funky right now. Only one caller "consumes" an
        // event, so that multiple things don't all respond to one keypress, but that's set up
        // oddly for update events.)
        if let Some(dt) = ctx.input.nonblocking_is_update_event() {
            ctx.input.use_update_event();
            self.elapsed += dt;
            self.controls.replace(
                ctx,
                "stopwatch",
                format!("Stopwatch: {}", self.elapsed)
                    .draw_text(ctx)
                    .named("stopwatch"),
            );
        }

        if self.controls.is_checked("Show timeseries") {
            // Update the panel when time changes.
            if self
                .timeseries_panel
                .as_ref()
                .map(|(dt, _)| *dt != self.elapsed)
                .unwrap_or(true)
            {
                self.timeseries_panel = Some((self.elapsed, self.make_timeseries_panel(ctx)));
            }
        } else {
            self.timeseries_panel = None;
        }

        if let Some((_, ref mut p)) = self.timeseries_panel {
            match p.event(ctx) {
                // No buttons in there
                Some(Outcome::Clicked(_)) => unreachable!(),
                None => {}
            }
        }

        // If we're paused, only call event() again when there's some kind of input. If not, also
        // sprinkle in periodic update events as time passes.
        if self.controls.is_checked("paused") {
            EventLoopMode::InputOnly
        } else {
            EventLoopMode::Animation
        }
    }

    fn draw(&self, g: &mut GfxCtx) {
        g.clear(Color::BLACK);

        if self.controls.is_checked("Draw scrollable canvas") {
            g.redraw(&self.scrollable_canvas);
        }

        self.controls.draw(g);

        if let Some((_, ref p)) = self.timeseries_panel {
            p.draw(g);
        }
    }
}

// This prepares a bunch of geometry (colored polygons) and uploads it to the GPU once. Then it can
// be redrawn cheaply later.
fn setup_scrollable_canvas(ctx: &mut EventCtx) -> Drawable {
    let mut batch = GeomBatch::new();
    batch.push(
        Color::hex("#4E30A6"),
        Polygon::rounded_rectangle(5000.0, 5000.0, 25.0),
    );
    // SVG support using lyon and usvg.
    batch.add_svg(
        &ctx.prerender,
        "../data/system/assets/pregame/logo.svg",
        // Translate
        Pt2D::new(300.0, 300.0),
        // Scale
        1.0,
        // Rotate
        Angle::ZERO,
    );
    // Text rendering also goes through lyon and usvg.
    batch.add_transformed(
        Text::from(Line("Awesome vector text thanks to usvg and lyon").fg(Color::hex("#DF8C3D")))
            .render_to_batch(&ctx.prerender),
        // Translate
        Pt2D::new(600.0, 500.0),
        // Scale
        2.0,
        // Rotate
        Angle::new_degs(30.0),
    );
    // This is a bit of a hack; it's needed so that zooming in/out has reasonable limits.
    ctx.canvas.map_dims = (5000.0, 5000.0);
    batch.upload(ctx)
}

fn make_controls(ctx: &mut EventCtx) -> Composite {
    Composite::new(
        ManagedWidget::col(vec![
            {
                let mut txt = Text::from(Line("ezgui demo").roboto_bold());
                txt.add(Line(
                    "Click and drag to pan, use touchpad or scroll wheel to zoom",
                ));
                txt.draw(ctx)
            },
            ManagedWidget::row(vec![
                // This just cycles between two arbitrary buttons
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
                // This is pretty verbose to create buttons. In A/B Street, I have a bunch of
                // helpers to create buttons with pre-set styles. I'm not sure if there should be a
                // generic library to apply styles to things.
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
                ManagedWidget::checkbox(ctx, "Draw scrollable canvas", None, true).margin(5),
                ManagedWidget::checkbox(ctx, "Show timeseries", lctrl(Key::T), false).margin(5),
            ])
            .evenly_spaced(),
            "Stopwatch: ...".draw_text(ctx).named("stopwatch"),
        ])
        .bg(Color::grey(0.4)),
    )
    .aligned(HorizontalAlignment::Center, VerticalAlignment::Top)
    .build(ctx)
}
