use std::collections::HashSet;

use rand::SeedableRng;
use rand_xorshift::XorShiftRng;

use geom::{Angle, Duration, Percent, Polygon, Pt2D, Time};
use widgetry::{
    lctrl, Btn, Checkbox, Choice, Color, Drawable, EventCtx, Fill, GeomBatch, GfxCtx,
    HorizontalAlignment, Key, Line, LinePlot, Outcome, Panel, PersistentSplit, PlotOptions, Series,
    SharedAppState, State, Text, TextExt, Texture, Transition, UpdateType, VerticalAlignment,
    Widget,
};

pub fn main() {
    // Use this to initialize logging.
    abstutil::CmdArgs::new().done();

    // Control flow surrendered here. App implements State, which has an event handler and a draw
    // callback.
    widgetry::run(widgetry::Settings::new("widgetry demo"), |ctx| {
        (App {}, vec![Box::new(Demo::new(ctx))])
    });
}

struct App {}

impl SharedAppState for App {}

struct Demo {
    controls: Panel,
    timeseries_panel: Option<(Duration, Panel)>,
    scrollable_canvas: Drawable,
    texture_demo: Drawable,
    elapsed: Duration,
}

impl Demo {
    fn new(ctx: &mut EventCtx) -> Demo {
        Demo {
            controls: make_controls(ctx),
            timeseries_panel: None,
            scrollable_canvas: setup_scrollable_canvas(ctx),
            texture_demo: setup_texture_demo(ctx, Texture::SAND, Texture::CACTUS),
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
                Line(format!("{}", Duration::seconds(s as f64)))
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

    fn redraw_stopwatch(&mut self, ctx: &mut EventCtx) {
        // We can replace any named widget with another one. Layout gets recalculated.
        self.controls.replace(
            ctx,
            "stopwatch",
            format!("Stopwatch: {}", self.elapsed).draw_text(ctx),
        );
    }
}

impl State<App> for Demo {
    fn event(&mut self, ctx: &mut EventCtx, _: &mut App) -> Transition<App> {
        // Allow panning and zooming to work.
        ctx.canvas_movement();

        // This dispatches event handling to all of the widgets inside.
        match self.controls.event(ctx) {
            Outcome::Clicked(x) => match x.as_ref() {
                // These outcomes should probably be a custom enum per Panel, to be more
                // typesafe.
                "reset the stopwatch" => {
                    self.elapsed = Duration::ZERO;
                    self.redraw_stopwatch(ctx);
                }
                "generate new faces" => {
                    self.scrollable_canvas = setup_scrollable_canvas(ctx);
                }
                "adjust timer" => {
                    let offset: Duration = self.controls.persistent_split_value("adjust timer");
                    self.elapsed += offset;
                    self.redraw_stopwatch(ctx);
                }
                "apply" => {
                    let (v_align, h_align) = self.controls.dropdown_value("alignment");
                    self.controls.align(v_align, h_align);
                    let (bg_texture, fg_texture) = self.controls.dropdown_value("texture");
                    self.texture_demo = setup_texture_demo(ctx, bg_texture, fg_texture);
                }
                _ => unimplemented!("clicked: {:?}", x),
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
            self.redraw_stopwatch(ctx);
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

        Transition::Keep
    }

    fn draw(&self, g: &mut GfxCtx, _: &App) {
        g.clear(Color::BLACK);

        if self.controls.is_checked("Draw scrollable canvas") {
            g.redraw(&self.scrollable_canvas);
        }

        self.controls.draw(g);

        if let Some((_, ref p)) = self.timeseries_panel {
            p.draw(g);
        }

        g.redraw(&self.texture_demo);
    }
}

fn setup_texture_demo(ctx: &mut EventCtx, bg_texture: Texture, fg_texture: Texture) -> Drawable {
    let mut batch = GeomBatch::new();

    let mut rect = Polygon::rectangle(100.0, 100.0);
    rect = rect.translate(200.0, 900.0);
    // Texture::NOOP should always be pure white, since all "non-textured" colors are multiplied by
    // Texture::NOOP (Texture::NOOP.0 == 0)
    batch.push(Texture::NOOP, rect);

    let triangle = geom::Triangle {
        pt1: Pt2D::new(0.0, 100.0),
        pt2: Pt2D::new(50.0, 0.0),
        pt3: Pt2D::new(100.0, 100.0),
    };
    let mut triangle_poly = Polygon::from_triangle(&triangle);
    triangle_poly = triangle_poly.translate(400.0, 900.0);
    batch.push(bg_texture, triangle_poly);

    let circle = geom::Circle::new(Pt2D::new(50.0, 50.0), geom::Distance::meters(50.0));
    let mut circle_poly = circle.to_polygon();
    circle_poly = circle_poly.translate(600.0, 900.0);
    batch.push(
        Fill::ColoredTexture(Color::RED, bg_texture),
        circle_poly.clone(),
    );
    batch.push(fg_texture, circle_poly.clone());

    batch.upload(ctx)
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
    batch
        .append(GeomBatch::load_svg(ctx, "system/assets/pregame/logo.svg").translate(300.0, 300.0));
    // Text rendering also goes through lyon and usvg.
    batch.append(
        Text::from(Line("Awesome vector text thanks to usvg and lyon").fg(Color::hex("#DF8C3D")))
            .render_autocropped(ctx)
            .scale(2.0)
            .centered_on(Pt2D::new(600.0, 500.0))
            .rotate(Angle::degrees(-30.0)),
    );

    let mut rng = if cfg!(target_arch = "wasm32") {
        XorShiftRng::seed_from_u64(0)
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
            Btn::text_fg("New faces").build(ctx, "generate new faces", Key::F),
            Checkbox::switch(ctx, "Draw scrollable canvas", None, true),
            Checkbox::switch(ctx, "Show timeseries", lctrl(Key::T), false),
        ]),
        "Stopwatch: ..."
            .draw_text(ctx)
            .named("stopwatch")
            .margin_above(30),
        Widget::row(vec![
            Checkbox::new(
                false,
                Btn::text_bg1("Pause").build(ctx, "pause the stopwatch", Key::Space),
                Btn::text_bg1("Resume").build(ctx, "resume the stopwatch", Key::Space),
            )
            .named("paused"),
            PersistentSplit::new(
                ctx,
                "adjust timer",
                Duration::seconds(20.0),
                None,
                vec![
                    Choice::new("+20s", Duration::seconds(20.0)),
                    Choice::new("-10s", Duration::seconds(-10.0)),
                ],
            ),
            Btn::text_fg("Reset Timer").build(ctx, "reset the stopwatch", None),
        ])
        .evenly_spaced(),
        Widget::row(vec![
            Widget::dropdown(
                ctx,
                "alignment",
                (HorizontalAlignment::Center, VerticalAlignment::Top),
                vec![
                    Choice::new("Top", (HorizontalAlignment::Center, VerticalAlignment::Top)),
                    Choice::new(
                        "Left",
                        (HorizontalAlignment::Left, VerticalAlignment::Center),
                    ),
                    Choice::new(
                        "Bottom",
                        (HorizontalAlignment::Center, VerticalAlignment::Bottom),
                    ),
                    Choice::new(
                        "Right",
                        (HorizontalAlignment::Right, VerticalAlignment::Center),
                    ),
                    Choice::new(
                        "Center",
                        (HorizontalAlignment::Center, VerticalAlignment::Center),
                    ),
                ],
            ),
            Widget::dropdown(
                ctx,
                "texture",
                (Texture::SAND, Texture::CACTUS),
                vec![
                    Choice::new("Cold", (Texture::SNOW, Texture::SNOW_PERSON)),
                    Choice::new("Hot", (Texture::SAND, Texture::CACTUS)),
                ],
            ),
            Btn::text_fg("Apply").build(ctx, "apply", None),
        ])
        .margin_above(30),
    ]))
    .aligned(HorizontalAlignment::Center, VerticalAlignment::Top)
    .build(ctx)
}

// Boilerplate for web support

#[cfg(target_arch = "wasm32")]
use wasm_bindgen::prelude::*;

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen(start)]
pub fn run() {
    main();
}
