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
}

impl ToolPanel {
    pub fn new(ctx: &EventCtx) -> ToolPanel {
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
        layout::stack_horizontally(top_left, 5.0, vec![&mut home_btn, &mut settings_btn]);

        ToolPanel {
            bg,
            rect: ScreenRectangle::top_left(top_left, ScreenDims::new(width, height)),
            home_btn,
            settings_btn,
        }
    }

    pub fn draw(&self, g: &mut GfxCtx) {
        self.bg.draw(g);
        self.home_btn.draw(g);
        self.settings_btn.draw(g);
        g.canvas.mark_covered_area(self.rect.clone());
    }
}
