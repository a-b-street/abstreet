use crate::common::{navigate, shortcuts};
use crate::game::Transition;
use crate::managed::{Composite, ManagedWidget};
use crate::options;
use ezgui::{hotkey, Button, Color, EventCtx, Key, RewriteColor, ScreenPt};

// TODO Rethink this API.
pub fn tool_panel(ctx: &EventCtx, extra_buttons: Vec<ManagedWidget>) -> Composite {
    let mut row = vec![
        // TODO Maybe this is confusing -- it doesn't jump to the title screen necessarily.
        // Caller has to handle this one
        ManagedWidget::btn_no_cb(Button::rectangle_svg(
            "assets/tools/home.svg",
            "back",
            hotkey(Key::Escape),
            RewriteColor::ChangeAll(Color::ORANGE),
            ctx,
        )),
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
    row.extend(extra_buttons);

    Composite::minimal_size(
        ManagedWidget::row(row.into_iter().map(|x| x.margin(10)).collect()).bg(Color::grey(0.4)),
        ScreenPt::new(30.0, ctx.canvas.window_height - 80.0),
    )
}
