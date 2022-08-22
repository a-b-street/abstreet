use widgetry::{
    ControlState, EventCtx, GfxCtx, HorizontalAlignment, Image, Key, Outcome, Panel,
    VerticalAlignment, Widget,
};

use crate::Transition;

// Partly copied from ungap/layers.s

pub struct Layers {
    panel: Panel,
    minimized: bool,
    zoom_enabled_cache_key: (bool, bool),
}

impl Layers {
    pub fn new(ctx: &mut EventCtx) -> Layers {
        let mut l = Layers {
            panel: Panel::empty(ctx),
            minimized: true,
            zoom_enabled_cache_key: zoom_enabled_cache_key(ctx),
        };
        l.update_panel(ctx);
        l
    }

    pub fn event(&mut self, ctx: &mut EventCtx) -> Option<Transition> {
        match self.panel.event(ctx) {
            Outcome::Clicked(x) => {
                match x.as_ref() {
                    "zoom map out" => {
                        ctx.canvas.center_zoom(-8.0);
                        self.update_panel(ctx);
                    }
                    "zoom map in" => {
                        ctx.canvas.center_zoom(8.0);
                        self.update_panel(ctx);
                    }
                    "hide panel" => {
                        self.minimized = true;
                        self.update_panel(ctx);
                    }
                    "show panel" => {
                        self.minimized = false;
                        self.update_panel(ctx);
                    }
                    _ => unreachable!(),
                }
                return Some(Transition::Keep);
            }
            _ => {}
        }

        if self.zoom_enabled_cache_key != zoom_enabled_cache_key(ctx) {
            self.update_panel(ctx);
            self.zoom_enabled_cache_key = zoom_enabled_cache_key(ctx);
        }

        None
    }

    pub fn draw(&self, g: &mut GfxCtx) {
        self.panel.draw(g);
    }

    fn update_panel(&mut self, ctx: &mut EventCtx) {
        self.panel = Panel::new_builder(
            Widget::col(vec![
                make_zoom_controls(ctx).align_right(),
                self.make_legend(ctx).bg(ctx.style().panel_bg),
            ])
            .padding_right(16),
        )
        .aligned(HorizontalAlignment::Right, VerticalAlignment::Bottom)
        .build_custom(ctx);
    }

    fn make_legend(&self, ctx: &mut EventCtx) -> Widget {
        if self.minimized {
            return ctx
                .style()
                .btn_plain
                .icon("system/assets/tools/layers.svg")
                .hotkey(Key::L)
                .build_widget(ctx, "show panel")
                .centered_horiz();
        }

        Widget::col(vec![Widget::row(vec![
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
        ])])
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
