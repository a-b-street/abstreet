use crate::common::{navigate, shortcuts};
use crate::game::Transition;
use crate::managed::{Callback, Composite, ManagedWidget};
use crate::options;
use ezgui::{hotkey, Button, Color, EventCtx, Key, RewriteColor, ScreenPt};

pub fn tool_panel(ctx: &EventCtx, layers_callback: Option<Callback>) -> Composite {
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
    if let Some(cb) = layers_callback {
        row.push(ManagedWidget::svg_button(
            ctx,
            "assets/tools/layers.svg",
            "change overlay",
            hotkey(Key::L),
            cb,
        ));
    }

    Composite::minimal_size(
        ManagedWidget::row(row)
            .padding(10)
            .bg(Color::grey(0.4))
            .min_width(200)
            .evenly_spaced(),
        ScreenPt::new(30.0, ctx.canvas.window_height - 80.0),
    )
}
