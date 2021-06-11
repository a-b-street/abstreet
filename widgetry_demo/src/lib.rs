use std::collections::HashSet;

use rand::SeedableRng;
use rand_xorshift::XorShiftRng;

use geom::{Angle, Duration, Percent, Polygon, Pt2D, Time};
use widgetry::{
    lctrl, Choice, Color, ContentMode, DragDrop, Drawable, EventCtx, Fill, GeomBatch, GfxCtx,
    HorizontalAlignment, Image, Key, Line, LinePlot, Outcome, Panel, PersistentSplit, PlotOptions,
    ScreenDims, Series, Settings, SharedAppState, State, TabController, Text, TextExt, Texture,
    Toggle, Transition, UpdateType, VerticalAlignment, Widget,
};

pub fn main() {
    // Use this to initialize logging.
    abstutil::CmdArgs::new().done();

    let settings = Settings::new("widgetry demo");
    run(settings);
}

fn run(mut settings: Settings) {
    settings = settings.read_svg(Box::new(abstio::slurp_bytes));
    // Control flow surrendered here. App implements State, which has an event handler and a draw
    // callback.
    //
    // TODO The demo loads a .svg file, so to make it work on both native and web, for now we use
    // read_svg. But we should have a more minimal example of how to do that here.
    widgetry::run(settings, |ctx| {
        // TODO: remove Style::pregame and make light_bg the default.
        ctx.set_style(widgetry::Style::light_bg());
        // TODO: Add a toggle to switch theme in demo (and recreate UI in that new theme)
        // ctx.set_style(widgetry::Style::dark_bg());

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
    tabs: TabController,
}

impl Demo {
    fn new(ctx: &mut EventCtx) -> Self {
        let mut tabs = make_tabs(ctx);
        Self {
            controls: make_controls(ctx, &mut tabs),
            timeseries_panel: None,
            scrollable_canvas: setup_scrollable_canvas(ctx),
            texture_demo: setup_texture_demo(ctx, Texture::SAND, Texture::CACTUS),
            elapsed: Duration::ZERO,
            tabs,
        }
    }

    fn make_timeseries_panel(&self, ctx: &mut EventCtx) -> Panel {
        // Make a table with 3 columns.
        let mut col1 = vec![Line("Time").into_widget(ctx)];
        let mut col = vec![Line("Linear").into_widget(ctx)];
        let mut col3 = vec![Line("Quadratic").into_widget(ctx)];
        for s in 0..(self.elapsed.inner_seconds() as usize) {
            col1.push(
                Line(format!("{}", Duration::seconds(s as f64)))
                    .secondary()
                    .into_widget(ctx),
            );
            col.push(Line(s.to_string()).secondary().into_widget(ctx));
            col3.push(Line(s.pow(2).to_string()).secondary().into_widget(ctx));
        }

        let mut c = Panel::new_builder(Widget::col(vec![
            Text::from_multiline(vec![
                Line("Here's a bunch of text to force some scrolling.").small_heading(),
                Line(
                    "Bug: scrolling by clicking and dragging doesn't work while the stopwatch is \
                     running.",
                )
                .fg(Color::RED),
            ])
            .into_widget(ctx),
            Widget::row(vec![
                // Examples of styling widgets
                Widget::col(col1).outline((2.0, Color::BLACK)).padding(5),
                Widget::col(col).outline((5.0, Color::BLACK)).padding(5),
                Widget::col(col3).outline((5.0, Color::BLUE)).padding(5),
            ]),
            LinePlot::new_widget(
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
            format!("Stopwatch: {}", self.elapsed).text_widget(ctx),
        );
    }
}

impl State<App> for Demo {
    fn event(&mut self, ctx: &mut EventCtx, _: &mut App) -> Transition<App> {
        // Allow panning and zooming to work.
        ctx.canvas_movement();

        // This dispatches event handling to all of the widgets inside.
        if let Outcome::Clicked(x) = self.controls.event(ctx) {
            match x.as_ref() {
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
                action => {
                    if self.tabs.handle_action(ctx, action, &mut self.controls) {
                        // if true, tabs has handled the action
                    } else if action.contains("btn_") {
                        log::info!("clicked button: {:?}", action);
                    } else {
                        unimplemented!("clicked: {:?}", x);
                    }
                }
            }
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
            if let Outcome::Clicked(_) = p.event(ctx) {
                unreachable!()
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
    batch.push(fg_texture, circle_poly);

    batch.upload(ctx)
}

// This prepares a bunch of geometry (colored polygons) and uploads it to the GPU once. Then it can
// be redrawn cheaply later.
fn setup_scrollable_canvas(ctx: &mut EventCtx) -> Drawable {
    let mut batch = GeomBatch::new();
    batch.push(
        Color::hex("#4E30A6"),
        Polygon::rounded_rectangle(5000.0, 5000.0, 25.0),
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
        let face = GeomBatch::load_svg_bytes_uncached(&svg_data).autocrop();
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

fn make_tabs(ctx: &mut EventCtx) -> TabController {
    let draggable_cards = (0..5)
        .map(|i| make_draggable_card(ctx, i))
        .collect::<Vec<_>>();
    let style = ctx.style();

    let mut tabs = TabController::new("demo_tabs");

    let gallery_bar_item = style.btn_tab.text("Component Gallery");
    let gallery_content = Widget::col(vec![
        // TODO Move this to the bottom
        "Reorder the cards below".text_widget(ctx),
        DragDrop::new_widget(ctx, "cards", draggable_cards),
        Text::from(Line("Text").big_heading_styled().size(18)).into_widget(ctx),
        Text::from_all(vec![
            Line("You can "),
            Line("change fonts ").big_heading_plain(),
            Line("on the same ").small().fg(Color::BLUE),
            Line("line!").small_heading(),
        ])
        .bg(Color::GREEN)
        .into_widget(ctx),
        // Button Style Gallery
        Text::from(Line("Buttons").big_heading_styled().size(18)).into_widget(ctx),
        Widget::row(vec![
            style
                .btn_solid_primary
                .text("Primary")
                .build_widget(ctx, "btn_solid_primary_text"),
            Widget::row(vec![
                style
                    .btn_solid_primary
                    .icon("system/assets/tools/map.svg")
                    .build_widget(ctx, "btn_solid_primary_icon"),
                style
                    .btn_plain_primary
                    .icon("system/assets/tools/map.svg")
                    .build_widget(ctx, "btn_plain_primary_icon"),
            ]),
            style
                .btn_solid_primary
                .icon_text("system/assets/tools/location.svg", "Primary")
                .build_widget(ctx, "btn_solid_primary_icon_text"),
        ]),
        Widget::row(vec![
            style
                .btn_outline
                .text("Secondary")
                .build_widget(ctx, "btn_outline_text"),
            Widget::row(vec![
                style
                    .btn_outline
                    .icon("system/assets/tools/map.svg")
                    .build_widget(ctx, "btn_outline_icon"),
                style
                    .btn_plain
                    .icon("system/assets/tools/map.svg")
                    .build_widget(ctx, "btn_plain_icon"),
            ]),
            style
                .btn_outline
                .icon_text("system/assets/tools/home.svg", "Secondary")
                .build_widget(ctx, "btn_outline.icon_text"),
        ]),
        Widget::row(vec![style
            .btn_popup_icon_text("system/assets/tools/map.svg", "Popup")
            .build_widget(ctx, "btn_popup_icon_text")]),
        Text::from_multiline(vec![
            Line("Images").big_heading_styled().size(18),
            Line(
                "Images can be colored, scaled, and stretched. They can have a background and \
                 padding.",
            ),
        ])
        .into_widget(ctx),
        Widget::row(vec![
            Image::from_path("system/assets/tools/home.svg").into_widget(ctx),
            Image::from_path("system/assets/tools/home.svg")
                .color(Color::ORANGE)
                .bg_color(Color::BLACK)
                .dims(50.0)
                .into_widget(ctx),
            Image::from_path("system/assets/tools/home.svg")
                .color(Color::RED)
                .bg_color(Color::BLACK)
                .padding(20)
                .dims(ScreenDims::new(50.0, 100.0))
                .content_mode(ContentMode::ScaleAspectFit)
                .tooltip(
                    "With ScaleAspectFit content grows, without distorting its aspect ratio, \
                     until it reaches its padding bounds.",
                )
                .into_widget(ctx),
            Image::from_path("system/assets/tools/home.svg")
                .color(Color::GREEN)
                .bg_color(Color::PURPLE)
                .padding(20)
                .dims(ScreenDims::new(50.0, 100.0))
                .content_mode(ContentMode::ScaleToFill)
                .tooltip("With ScaleToFill content can stretches to fill its size (less padding)")
                .into_widget(ctx),
            Image::from_path("system/assets/tools/home.svg")
                .color(Color::BLUE)
                .bg_color(Color::YELLOW)
                .padding(20)
                .dims(ScreenDims::new(50.0, 100.0))
                .content_mode(ContentMode::ScaleAspectFill)
                .tooltip("With ScaleAspectFill content can exceed its visible bounds")
                .into_widget(ctx),
        ]),
        Text::from(Line("Spinner").big_heading_styled().size(18)).into_widget(ctx),
        widgetry::Spinner::widget(ctx, "spinner", (0, 11), 1, 1),
    ]);
    tabs.push_tab(gallery_bar_item, gallery_content);

    let qa_bar_item = style.btn_tab.text("Conformance Checks");
    let qa_content = Widget::col(vec![
        Text::from(
            Line("Controls should be same height")
                .big_heading_styled()
                .size(18),
        )
        .into_widget(ctx),
        {
            let row_height = 10;
            let mut id = 0;
            let mut next_id = || {
                id += 1;
                format!("btn_height_check_{}", id)
            };
            Widget::row(vec![
                Widget::col(
                    (0..row_height)
                        .map(|_| {
                            style
                                .btn_outline
                                .icon("system/assets/tools/layers.svg")
                                .build_widget(ctx, &next_id())
                        })
                        .collect::<Vec<_>>(),
                ),
                Widget::col(
                    (0..row_height)
                        .map(|_| style.btn_outline.text("text").build_widget(ctx, &next_id()))
                        .collect::<Vec<_>>(),
                ),
                Widget::col(
                    (0..row_height)
                        .map(|_| {
                            style
                                .btn_outline
                                .icon_text("system/assets/tools/layers.svg", "icon+text")
                                .build_widget(ctx, &next_id())
                        })
                        .collect::<Vec<_>>(),
                ),
                Widget::col(
                    (0..row_height)
                        .map(|_| {
                            style
                                .btn_popup_icon_text("system/assets/tools/layers.svg", "icon+text")
                                .build_widget(ctx, &next_id())
                        })
                        .collect::<Vec<_>>(),
                ),
                Widget::col(
                    (0..row_height)
                        .map(|_| {
                            style
                                .btn_outline
                                .popup("popup")
                                .build_widget(ctx, &next_id())
                        })
                        .collect::<Vec<_>>(),
                ),
                Widget::col(
                    (0..row_height)
                        .map(|i| {
                            widgetry::Spinner::widget(ctx, format!("spinner {}", i), (0, 11), 1, 1)
                        })
                        .collect::<Vec<_>>(),
                ),
                Widget::col(
                    (0..row_height)
                        .map(|_| widgetry::Toggle::checkbox(ctx, "checkbox", None, true))
                        .collect::<Vec<_>>(),
                ),
            ])
        },
    ]);

    tabs.push_tab(qa_bar_item, qa_content);

    tabs
}

fn make_controls(ctx: &mut EventCtx, tabs: &mut TabController) -> Panel {
    Panel::new_builder(Widget::col(vec![
        Text::from(Line("widgetry demo").big_heading_styled()).into_widget(ctx),
        Widget::col(vec![
            Text::from(
                "Click and drag the background to pan, use touchpad or scroll wheel to zoom",
            )
            .into_widget(ctx),
            Widget::row(vec![
                ctx.style()
                    .btn_outline
                    .text("New faces")
                    .hotkey(Key::F)
                    .build_widget(ctx, "generate new faces"),
                Toggle::switch(ctx, "Draw scrollable canvas", None, true),
                Toggle::switch(ctx, "Show timeseries", lctrl(Key::T), false),
            ]),
            "Stopwatch: ..."
                .text_widget(ctx)
                .named("stopwatch")
                .margin_above(30),
            Widget::row(vec![
                Toggle::new_widget(
                    false,
                    ctx.style()
                        .btn_outline
                        .text("Pause")
                        .hotkey(Key::Space)
                        .build(ctx, "pause the stopwatch"),
                    ctx.style()
                        .btn_outline
                        .text("Resume")
                        .hotkey(Key::Space)
                        .build(ctx, "resume the stopwatch"),
                )
                .named("paused"),
                PersistentSplit::widget(
                    ctx,
                    "adjust timer",
                    Duration::seconds(20.0),
                    None,
                    vec![
                        Choice::new("+20s", Duration::seconds(20.0)),
                        Choice::new("-10s", Duration::seconds(-10.0)),
                    ],
                ),
                ctx.style()
                    .btn_outline
                    .text("Reset Timer")
                    .build_widget(ctx, "reset the stopwatch"),
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
                ctx.style()
                    .btn_solid_primary
                    .text("Apply")
                    .build_widget(ctx, "apply"),
            ])
            .margin_above(30),
        ])
        .section(ctx),
        tabs.build_widget(ctx),
    ])) // end panel
    .aligned(HorizontalAlignment::Center, VerticalAlignment::Top)
    .build(ctx)
}

fn make_draggable_card(ctx: &mut EventCtx, num: usize) -> GeomBatch {
    // TODO Kind of hardcoded. At least center the text or draw nice outlines?
    let mut batch = GeomBatch::new();
    batch.push(Color::RED, Polygon::rectangle(100.0, 150.0));
    batch.append(Text::from(format!("Card {}", num)).render(ctx));
    batch
}

// Boilerplate for web support

#[cfg(target_arch = "wasm32")]
use wasm_bindgen::prelude::*;

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen(js_name = "run")]
pub fn run_wasm(root_dom_id: String, assets_base_url: String, assets_are_gzipped: bool) {
    // Use this to initialize logging.
    abstutil::CmdArgs::new().done();
    let settings = Settings::new("widgetry demo")
        .root_dom_element_id(root_dom_id)
        .assets_base_url(assets_base_url)
        .assets_are_gzipped(assets_are_gzipped);
    run(settings);
}
