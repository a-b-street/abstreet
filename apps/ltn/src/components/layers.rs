use geom::Polygon;
use map_gui::colors::ColorScheme;
use widgetry::tools::ColorLegend;
use widgetry::{
    ButtonBuilder, Color, ControlState, EdgeInsets, EventCtx, GeomBatch, GfxCtx,
    HorizontalAlignment, Image, Key, Line, Outcome, Panel, RoundedF64, Spinner, TextExt, Toggle,
    VerticalAlignment, Widget,
};

use crate::components::Mode;
use crate::{colors, App, FilterType, Transition};

// Partly copied from ungap/layers.s

pub struct Layers {
    panel: Panel,
    minimized: bool,
    // (Mode, max zoom, min zoom, bottom bar position)
    panel_cache_key: (Mode, bool, bool, Option<f64>),
    show_bus_routes: bool,
}

impl Layers {
    /// Panel won't be initialized, must call `event` first
    pub fn new(ctx: &mut EventCtx) -> Layers {
        Self {
            panel: Panel::empty(ctx),
            minimized: true,
            panel_cache_key: (Mode::Impact, false, false, None),
            show_bus_routes: false,
        }
    }

    pub fn event(
        &mut self,
        ctx: &mut EventCtx,
        cs: &ColorScheme,
        mode: Mode,
        bottom_panel: Option<&Panel>,
    ) -> Option<Transition> {
        match self.panel.event(ctx) {
            Outcome::Clicked(x) => {
                match x.as_ref() {
                    "zoom map out" => {
                        ctx.canvas.center_zoom(-8.0);
                    }
                    "zoom map in" => {
                        ctx.canvas.center_zoom(8.0);
                    }
                    "hide layers" => {
                        self.minimized = true;
                    }
                    "show layers" => {
                        self.minimized = false;
                    }
                    _ => unreachable!(),
                }
                self.update_panel(ctx, cs, bottom_panel);
                return Some(Transition::Keep);
            }
            Outcome::Changed(x) => {
                if x == "show bus routes" {
                    self.show_bus_routes = self.panel.is_checked("show bus routes");
                    self.update_panel(ctx, cs, bottom_panel);
                    return Some(Transition::Keep);
                }

                ctx.set_scale_factor(self.panel.spinner::<RoundedF64>("scale_factor").0);
                // TODO This doesn't seem to do mark_covered_area correctly, so using the scroll
                // wheel on the spinner just scrolls the canvas
                self.update_panel(ctx, cs, bottom_panel);
                return Some(Transition::Recreate);
            }
            _ => {}
        }

        let cache_key = (mode, ctx.canvas.is_max_zoom(), ctx.canvas.is_min_zoom(), bottom_panel.map(|p| p.panel_rect().y1));
        if self.panel_cache_key != cache_key {
            self.panel_cache_key = cache_key;
            self.update_panel(ctx, cs, bottom_panel);
        }

        None
    }

    pub fn draw(&self, g: &mut GfxCtx, app: &App) {
        self.panel.draw(g);
        if self.show_bus_routes {
            g.redraw(&app.per_map.draw_bus_routes);
        }
    }

    pub fn show_bus_routes(
        &mut self,
        ctx: &mut EventCtx,
        cs: &ColorScheme,
        bottom_panel: Option<&Panel>,
    ) {
        self.minimized = false;
        self.show_bus_routes = true;
        self.update_panel(ctx, cs, bottom_panel);
    }

    fn update_panel(&mut self, ctx: &mut EventCtx, cs: &ColorScheme, bottom_panel: Option<&Panel>) {
        let mut builder = Panel::new_builder(
            Widget::col(vec![
                make_zoom_controls(ctx).align_right(),
                self.make_legend(ctx, cs).bg(ctx.style().panel_bg),
            ])
            .padding_right(16),
        )
        .aligned(HorizontalAlignment::Right, VerticalAlignment::Bottom);
        if let Some(bottom_panel) = bottom_panel {
            let buffer = 5.0;
            builder = builder.aligned(
                HorizontalAlignment::Right,
                VerticalAlignment::Above(bottom_panel.panel_rect().y1 - buffer),
            );
        }
        self.panel = builder.build_custom(ctx);
    }

    fn make_legend(&self, ctx: &mut EventCtx, cs: &ColorScheme) -> Widget {
        if self.minimized {
            return ctx
                .style()
                .btn_plain
                .icon("system/assets/tools/layers.svg")
                .hotkey(Key::L)
                .build_widget(ctx, "show layers")
                .centered_horiz();
        }

        Widget::col(vec![
            Widget::row(vec![
                Image::from_path("system/assets/tools/layers.svg")
                    .dims(30.0)
                    .into_widget(ctx)
                    .centered_vert()
                    .named("layer icon"),
                ctx.style()
                    .btn_plain
                    .icon("system/assets/tools/minimize.svg")
                    .hotkey(Key::L)
                    .build_widget(ctx, "hide layers")
                    .align_right(),
            ]),
            self.panel_cache_key.0.legend(ctx, cs),
            {
                let checkbox = Toggle::checkbox(ctx, "show bus routes", None, self.show_bus_routes);
                if self.show_bus_routes {
                    checkbox.outline((1.0, *colors::BUS_ROUTE))
                } else {
                    checkbox
                }
            },
            Widget::row(vec![
                "Adjust the size of text:".text_widget(ctx).centered_vert(),
                Spinner::f64_widget(
                    ctx,
                    "scale_factor",
                    (0.5, 2.5),
                    ctx.prerender.get_scale_factor(),
                    0.1,
                ),
            ]),
        ])
        .padding(16)
    }
}

fn make_zoom_controls(ctx: &mut EventCtx) -> Widget {
    let builder = ctx
        .style()
        .btn_floating
        .btn()
        .image_dims(30.0)
        .outline((1.0, ctx.style().btn_plain.fg), ControlState::Default)
        .padding(12.0);

    Widget::custom_col(vec![
        builder
            .clone()
            .image_path("system/assets/speed/plus.svg")
            .corner_rounding(geom::CornerRadii {
                top_left: 16.0,
                top_right: 16.0,
                bottom_right: 0.0,
                bottom_left: 0.0,
            })
            .disabled(ctx.canvas.is_max_zoom())
            .build_widget(ctx, "zoom map in"),
        builder
            .image_path("system/assets/speed/minus.svg")
            .image_dims(30.0)
            .padding(12.0)
            .corner_rounding(geom::CornerRadii {
                top_left: 0.0,
                top_right: 0.0,
                bottom_right: 16.0,
                bottom_left: 16.0,
            })
            .disabled(ctx.canvas.is_min_zoom())
            .build_widget(ctx, "zoom map out"),
    ])
}

impl Mode {
    fn legend(&self, ctx: &mut EventCtx, cs: &ColorScheme) -> Widget {
        // TODO Light/dark buildings? Traffic signals?

        Widget::col(match self {
            Mode::PickArea => vec![
                entry(ctx, colors::HIGHLIGHT_BOUNDARY, "boundary road"),
                entry(ctx, Color::YELLOW.alpha(0.1), "neighbourhood"),
            ],
            Mode::ModifyNeighbourhood => vec![
                Widget::row(vec![
                    // TODO White = none
                    "Shortcuts:".text_widget(ctx),
                    ColorLegend::gradient_with_width(
                        ctx,
                        &cs.good_to_bad_red,
                        vec!["low", "high"],
                        150.0,
                    ),
                ]),
                Widget::row(vec!["Cells:".text_widget(ctx), color_grid(ctx)]),
                Widget::row(vec![
                    "Modal filters:".text_widget(ctx),
                    Image::from_path(FilterType::WalkCycleOnly.svg_path())
                        .untinted()
                        .dims(30.0)
                        .into_widget(ctx),
                    Image::from_path(FilterType::NoEntry.svg_path())
                        .untinted()
                        .dims(30.0)
                        .into_widget(ctx),
                    Image::from_path(FilterType::BusGate.svg_path())
                        .untinted()
                        .dims(30.0)
                        .into_widget(ctx),
                ]),
                Line("Faded filters exist already").small().into_widget(ctx),
                Widget::row(vec![
                    "Private road:".text_widget(ctx),
                    Image::from_path("system/assets/map/private_road.svg")
                        .untinted()
                        .dims(30.0)
                        .into_widget(ctx),
                ]),
                // TODO Entry/exit arrows?
                // TODO Dashed roads are walk/bike
            ],
            Mode::SelectBoundary => vec![
                entry(ctx, colors::HIGHLIGHT_BOUNDARY, "boundary road"),
                entry(
                    ctx,
                    colors::BLOCK_IN_BOUNDARY,
                    "block part of current neighbourhood",
                ),
                entry(
                    ctx,
                    colors::BLOCK_IN_FRONTIER,
                    "block could be added to current neighbourhood",
                ),
            ],
            Mode::RoutePlanner => vec![
                entry(
                    ctx,
                    *colors::PLAN_ROUTE_BEFORE,
                    "driving route before changes",
                ),
                entry(
                    ctx,
                    *colors::PLAN_ROUTE_AFTER,
                    "driving route after changes",
                ),
                entry(ctx, *colors::PLAN_ROUTE_BIKE, "cycling route"),
                // TODO Should we invert text color? This gets hard to read
                entry(ctx, *colors::PLAN_ROUTE_WALK, "walking route"),
                // TODO Highlighted roads are boundaries (or main?) roads
            ],
            Mode::Impact => vec![
                map_gui::tools::compare_counts::CompareCounts::relative_scale()
                    .make_legend(ctx, vec!["less", "same", "more"]),
            ],
        })
    }
}

fn entry(ctx: &mut EventCtx, color: Color, label: &'static str) -> Widget {
    ButtonBuilder::new()
        .label_text(label)
        .bg_color(color, ControlState::Disabled)
        .disabled(true)
        .padding(EdgeInsets {
            top: 10.0,
            bottom: 10.0,
            left: 20.0,
            right: 20.0,
        })
        .corner_rounding(0.0)
        .build_def(ctx)
}

fn color_grid(ctx: &mut EventCtx) -> Widget {
    let size = 16.0;
    let columns = 3;
    let mut batch = GeomBatch::new();

    for (i, color) in colors::CELLS.iter().enumerate() {
        let row = (i / columns) as f64;
        let column = (i % columns) as f64;
        batch.push(
            *color,
            Polygon::rectangle(size, size).translate(size * column, size * row),
        );
    }

    batch.into_widget(ctx)
}
