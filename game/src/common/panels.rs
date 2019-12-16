use crate::common::{navigate, shortcuts};
use crate::game::Transition;
use crate::managed::{Callback, Composite, ManagedWidget};
use crate::options;
use crate::ui::UI;
use ezgui::{hotkey, Button, Color, EventCtx, GfxCtx, Key, RewriteColor, ScreenPt};

// TODO Why wrap at all?
pub struct ToolPanel {
    composite: Composite,
}

impl ToolPanel {
    pub fn new(
        ctx: &EventCtx,
        home_btn_callback: Callback,
        layers_callback: Option<Callback>,
    ) -> ToolPanel {
        let mut row = vec![
            // TODO Maybe this is confusing -- it doesn't jump to the title screen necessarily.
            ManagedWidget::btn(
                Button::rectangle_svg(
                    "assets/tools/home.svg",
                    "back",
                    hotkey(Key::Escape),
                    RewriteColor::ChangeAll(Color::ORANGE),
                    ctx,
                ),
                home_btn_callback,
            ),
            ManagedWidget::btn(
                Button::rectangle_svg(
                    "assets/tools/settings.svg",
                    "settings",
                    None,
                    RewriteColor::ChangeAll(Color::ORANGE),
                    ctx,
                ),
                Box::new(|_, _| Some(Transition::Push(options::open_panel()))),
            ),
            ManagedWidget::svg_button(
                ctx,
                "assets/tools/search.svg",
                "search",
                hotkey(Key::K),
                Box::new(|_, ui| Some(Transition::Push(Box::new(navigate::Navigator::new(ui))))),
            ),
            ManagedWidget::svg_button(
                ctx,
                "assets/tools/shortcuts.svg",
                "shortcuts",
                hotkey(Key::SingleQuote),
                Box::new(|_, _| Some(Transition::Push(shortcuts::ChoosingShortcut::new()))),
            ),
        ];
        if let Some(cb) = layers_callback {
            row.push(ManagedWidget::svg_button(
                ctx,
                "assets/tools/layers.svg",
                "change overlay",
                hotkey(Key::L),
                cb,
            ));
        }

        ToolPanel {
            composite: Composite::minimal_size(
                ManagedWidget::row(row)
                    .padding(10)
                    .bg(Color::grey(0.4))
                    .min_width(200)
                    .evenly_spaced(),
                ScreenPt::new(30.0, ctx.canvas.window_height - 80.0),
            ),
        }
    }

    pub fn event(&mut self, ctx: &mut EventCtx, ui: &mut UI) -> Option<Transition> {
        self.composite.event(ctx, ui)
    }

    pub fn draw(&self, g: &mut GfxCtx) {
        self.composite.draw(g);
    }
}
