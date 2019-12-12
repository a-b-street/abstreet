use ezgui::layout::Widget;
use ezgui::{
    hotkey, layout, Button, Color, DrawBoth, EventCtx, GeomBatch, GfxCtx, JustDraw, Key,
    RewriteColor, ScreenDims, ScreenPt, ScreenRectangle,
};
use geom::{Distance, Polygon};

pub struct ToolPanel {
    bg: JustDraw,
    rect: ScreenRectangle,
    pub home_btn: Button,
    pub settings_btn: Button,
    // TODO These three belong by the minimap, but doing the layout change if the minimap is there
    // or not is hard right now.
    pub search_btn: Button,
    pub shortcuts_btn: Button,
    pub layers_btn: Option<Button>,
}

impl ToolPanel {
    pub fn new(ctx: &EventCtx, with_layers: bool) -> ToolPanel {
        let top_left = ScreenPt::new(30.0, ctx.canvas.window_height - 80.0);
        let width = 50.0;
        let height = 30.0;

        // TODO Hardcoding guessed dims
        let rect_bg = GeomBatch::from(vec![(
            Color::grey(0.4),
            Polygon::rounded_rectangle(
                Distance::meters(width),
                Distance::meters(height),
                Distance::meters(5.0),
            ),
        )]);
        let mut bg = JustDraw::wrap(DrawBoth::new(ctx, rect_bg, Vec::new()));
        bg.set_pos(top_left);

        // TODO Maybe this is confusing -- it doesn't jump to the title screen necessarily.
        let mut home_btn = Button::rectangle_svg(
            "assets/tools/home.svg",
            "back",
            hotkey(Key::Escape),
            RewriteColor::ChangeAll(Color::ORANGE),
            ctx,
        );
        let mut settings_btn = Button::rectangle_svg(
            "assets/tools/settings.svg",
            "settings",
            None,
            RewriteColor::ChangeAll(Color::ORANGE),
            ctx,
        );
        let mut search_btn = Button::rectangle_svg(
            "assets/tools/search.svg",
            "search",
            hotkey(Key::K),
            RewriteColor::Change(Color::WHITE, Color::ORANGE),
            ctx,
        );
        let mut shortcuts_btn = Button::rectangle_svg(
            "assets/tools/shortcuts.svg",
            "shortcuts",
            hotkey(Key::SingleQuote),
            RewriteColor::Change(Color::WHITE, Color::ORANGE),
            ctx,
        );
        let mut layers_btn = if with_layers {
            Some(Button::rectangle_svg(
                "assets/tools/layers.svg",
                "change overlay",
                hotkey(Key::L),
                RewriteColor::Change(Color::WHITE, Color::ORANGE),
                ctx,
            ))
        } else {
            None
        };
        let mut widgets: Vec<&mut dyn Widget> = vec![
            &mut home_btn,
            &mut settings_btn,
            &mut search_btn,
            &mut shortcuts_btn,
        ];
        if let Some(ref mut w) = layers_btn {
            widgets.push(w);
        }
        layout::stack_horizontally(top_left, 5.0, widgets);

        ToolPanel {
            bg,
            rect: ScreenRectangle::top_left(top_left, ScreenDims::new(width, height)),
            home_btn,
            settings_btn,
            search_btn,
            shortcuts_btn,
            layers_btn,
        }
    }

    pub fn event(&mut self, ctx: &mut EventCtx) {
        self.home_btn.event(ctx);
        self.settings_btn.event(ctx);
        self.search_btn.event(ctx);
        self.shortcuts_btn.event(ctx);
        if let Some(ref mut btn) = self.layers_btn {
            btn.event(ctx);
        }
    }

    pub fn draw(&self, g: &mut GfxCtx) {
        self.bg.draw(g);
        self.home_btn.draw(g);
        self.settings_btn.draw(g);
        self.search_btn.draw(g);
        self.shortcuts_btn.draw(g);
        if let Some(ref btn) = self.layers_btn {
            btn.draw(g);
        }
        g.canvas.mark_covered_area(self.rect.clone());
    }
}
