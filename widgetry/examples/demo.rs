// To run:
// > cargo run --example demo
//
// Try the web version, but there's no text rendering yet:
// > cargo web start --target wasm32-unknown-unknown --no-default-features \
// --features wasm-backend --example demo

use geom::{Angle, Duration, Percent, Polygon, Pt2D, Time};
use rand::SeedableRng;
use rand_xorshift::XorShiftRng;
use std::collections::HashSet;
use widgetry::{
    hotkey, lctrl, Btn, Checkbox, Color, Drawable, EventCtx, GeomBatch, GfxCtx,
    HorizontalAlignment, Key, Line, LinePlot, Outcome, Panel, PlotOptions, Series, Text, TextExt,
    UpdateType, VerticalAlignment, Widget, GUI,
};

fn main() {
    // Control flow surrendered here. App implements State, which has an event handler and a draw
    // callback.
    widgetry::run(widgetry::Settings::new("widgetry demo"), |ctx| {
        App::new(ctx)
    });
}

struct App {
    controls: Panel,
    timeseries_panel: Option<(Duration, Panel)>,
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

    fn make_timeseries_panel(&self, ctx: &mut EventCtx) -> Panel {
        // Make a table with 3 columns.
        let mut col1 = vec![Line("Time").draw(ctx)];
        let mut col = vec![Line("Linear").draw(ctx)];
        let mut col3 = vec![Line("Quadratic").draw(ctx)];
        for s in 0..(self.elapsed.inner_seconds() as usize) {
            col1.push(
                Line(Duration::seconds(s as f64).to_string())
                    .secondary()
                    .draw(ctx),
            );
            col.push(Line(s.to_string()).secondary().draw(ctx));
            col3.push(Line(s.pow(2).to_string()).secondary().draw(ctx));
        }

        let mut c = Panel::new(Widget::col(vec![
            Text::from_multiline(vec![
                Line("Here's a bunch of text to force some scrolling.").small_heading(),
                Line(
                    "Bug: scrolling by clicking and dragging doesn't work while the stopwatch is \
                     running.",
                )
                .fg(Color::RED),
            ])
            .draw(ctx),
            Widget::row(vec![
                // Examples of styling widgets
                Widget::col(col1).outline(3.0, Color::BLACK).padding(5),
                Widget::col(col).outline(3.0, Color::BLACK).padding(5),
                Widget::col(col3).outline(3.0, Color::BLACK).padding(5),
            ]),
            LinePlot::new(
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
                            .map(|s| (Time::START_OF_DAY + Duration::seconds(s as f64), s.pow(2)))
                            .collect(),
                    },
                ],
                PlotOptions {
                    filterable: false,
                    // Without this, the plot doesn't stretch to cover times in between whole
                    // seconds.
                    max_x: Some(Time::START_OF_DAY + self.elapsed),
                    max_y: None,
                    disabled: HashSet::new(),
                },
            ),
        ]))
        // Don't let the panel exceed this percentage of the window. Scrollbars appear
        // automatically if needed.
        .max_size(Percent::int(30), Percent::int(40))
        // We take up 30% width, and we want to leave 10% window width as buffer.
        .aligned(HorizontalAlignment::Percent(0.6), VerticalAlignment::Center)
        .build(ctx);

        // Since we're creating an entirely new panel when the time changes, we need to preserve
        // some internal state, like scroll and whether plot checkboxes were enabled.
        if let Some((_, ref old)) = self.timeseries_panel {
            c.restore(ctx, old);
        }
        c
    }
}

impl GUI for App {
    fn event(&mut self, ctx: &mut EventCtx) {
        // Allow panning and zooming to work.
        ctx.canvas_movement();

        // This dispatches event handling to all of the widgets inside.
        match self.controls.event(ctx) {
            Outcome::Clicked(x) => match x.as_ref() {
                // These outcomes should probably be a custom enum per Panel, to be more
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
                "generate new faces" => {
                    self.scrollable_canvas = setup_scrollable_canvas(ctx);
                }
                _ => unreachable!(),
            },
            _ => {}
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
                Outcome::Clicked(_) => unreachable!(),
                _ => {}
            }
        }

        // If we're paused, only call event() again when there's some kind of input. If not, also
        // sprinkle in periodic update events as time passes.
        if !self.controls.is_checked("paused") {
            ctx.request_update(UpdateType::Game);
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
        Polygon::rounded_rectangle(5000.0, 5000.0, Some(25.0)),
    );
    // SVG support using lyon and usvg. Map-space means don't scale for high DPI monitors.
    batch.append(
        GeomBatch::load_svg(&ctx.prerender, "system/assets/pregame/logo.svg")
            .translate(300.0, 300.0),
    );
    // Text rendering also goes through lyon and usvg.
    batch.append(
        Text::from(Line("Awesome vector text thanks to usvg and lyon").fg(Color::hex("#DF8C3D")))
            .render_to_batch(&ctx.prerender)
            .scale(2.0)
            .centered_on(Pt2D::new(600.0, 500.0))
            .rotate(Angle::new_degs(-30.0)),
    );

    let mut rng = if cfg!(target_arch = "wasm32") {
        XorShiftRng::from_seed([0; 16])
    } else {
        XorShiftRng::from_entropy()
    };
    for i in 0..10 {
        let mut svg_data = Vec::new();
        svg_face::generate_face(&mut svg_data, &mut rng).unwrap();
        let face = GeomBatch::from_svg_contents(svg_data).autocrop();
        let dims = face.get_dims();
        batch.append(
            face.scale((200.0 / dims.width).min(200.0 / dims.height))
                .translate(250.0 * (i as f64), 0.0),
        );
    }

    // This is a bit of a hack; it's needed so that zooming in/out has reasonable limits.
    ctx.canvas.map_dims = (5000.0, 5000.0);
    batch.upload(ctx)
}

fn make_controls(ctx: &mut EventCtx) -> Panel {
    Panel::new(Widget::col(vec![
        Text::from_multiline(vec![
            Line("widgetry demo").small_heading(),
            Line("Click and drag to pan, use touchpad or scroll wheel to zoom"),
        ])
        .draw(ctx),
        Widget::row(vec![
            // This just cycles between two arbitrary buttons
            Checkbox::new(
                false,
                Btn::text_bg1("Pause").build(ctx, "pause the stopwatch", hotkey(Key::Space)),
                Btn::text_bg1("Resume").build(ctx, "resume the stopwatch", hotkey(Key::Space)),
            )
            .named("paused"),
            Btn::text_fg("Reset timer").build(ctx, "reset the stopwatch", None),
            Btn::text_fg("New faces").build(ctx, "generate new faces", hotkey(Key::F)),
            Checkbox::switch(ctx, "Draw scrollable canvas", None, true),
            Checkbox::switch(ctx, "Show timeseries", lctrl(Key::T), false),
        ])
        .evenly_spaced(),
        "Stopwatch: ...".draw_text(ctx).named("stopwatch"),
    ]))
    .aligned(HorizontalAlignment::Center, VerticalAlignment::Top)
    .build(ctx)
}
