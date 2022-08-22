use geom::Polygon;
use map_gui::colors::ColorScheme;
use widgetry::tools::ColorLegend;
use widgetry::{
    ButtonBuilder, Color, ControlState, EdgeInsets, EventCtx, GeomBatch, GfxCtx,
    HorizontalAlignment, Image, Key, Outcome, Panel, TextExt, VerticalAlignment, Widget,
};

use crate::{colors, FilterType, Transition};

// Partly copied from ungap/layers.s

pub struct Layers {
    panel: Panel,
    minimized: bool,
    mode_cache_key: Mode,
    zoom_enabled_cache_key: (bool, bool),
}

impl Layers {
    /// Panel won't be initialized, must call `event` first
    pub fn new(ctx: &mut EventCtx) -> Layers {
        Self {
            panel: Panel::empty(ctx),
            minimized: true,
            mode_cache_key: Mode::Impact,
            zoom_enabled_cache_key: zoom_enabled_cache_key(ctx),
        }
    }

    pub fn event(
        &mut self,
        ctx: &mut EventCtx,
        cs: &ColorScheme,
        mode: Mode,
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
                    "hide panel" => {
                        self.minimized = true;
                    }
                    "show panel" => {
                        self.minimized = false;
                    }
                    _ => unreachable!(),
                }
                self.update_panel(ctx, cs);
                return Some(Transition::Keep);
            }
            _ => {}
        }

        if self.zoom_enabled_cache_key != zoom_enabled_cache_key(ctx) {
            self.zoom_enabled_cache_key = zoom_enabled_cache_key(ctx);
            self.update_panel(ctx, cs);
        }
        if self.mode_cache_key != mode {
            self.mode_cache_key = mode;
            self.update_panel(ctx, cs);
        }

        None
    }

    pub fn draw(&self, g: &mut GfxCtx) {
        self.panel.draw(g);
    }

    fn update_panel(&mut self, ctx: &mut EventCtx, cs: &ColorScheme) {
        self.panel = Panel::new_builder(
            Widget::col(vec![
                make_zoom_controls(ctx).align_right(),
                self.make_legend(ctx, cs).bg(ctx.style().panel_bg),
            ])
            .padding_right(16),
        )
        .aligned(HorizontalAlignment::Right, VerticalAlignment::Bottom)
        .build_custom(ctx);
    }

    fn make_legend(&self, ctx: &mut EventCtx, cs: &ColorScheme) -> Widget {
        if self.minimized {
            return ctx
                .style()
                .btn_plain
                .icon("system/assets/tools/layers.svg")
                .hotkey(Key::L)
                .build_widget(ctx, "show panel")
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
                    .build_widget(ctx, "hide panel")
                    .align_right(),
            ]),
            self.mode_cache_key.legend(ctx, cs),
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

fn zoom_enabled_cache_key(ctx: &EventCtx) -> (bool, bool) {
    (ctx.canvas.is_max_zoom(), ctx.canvas.is_min_zoom())
}

#[derive(PartialEq)]
pub enum Mode {
    BrowseNeighbourhoods,
    ModifyNeighbourhood,
    SelectBoundary,
    RoutePlanner,
    Impact,
}

impl Mode {
    fn legend(&self, ctx: &mut EventCtx, cs: &ColorScheme) -> Widget {
        // TODO Light/dark buildings? Traffic signals?

        Widget::col(match self {
            Mode::BrowseNeighbourhoods => vec![
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
                // TODO Entry/exit arrows?
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
