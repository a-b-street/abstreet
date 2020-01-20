use crate::game::Transition;
use crate::managed::Composite;
use crate::options;
use ezgui::{
    hotkey, Button, Color, EventCtx, HorizontalAlignment, Key, ManagedWidget, RewriteColor,
    VerticalAlignment,
};

pub fn tool_panel(ctx: &mut EventCtx) -> Composite {
    let row = vec![
        // TODO Maybe this is confusing -- it doesn't jump to the title screen necessarily.
        // Caller has to handle this one
        ManagedWidget::btn(Button::rectangle_svg(
            "assets/tools/home.svg",
            "back",
            hotkey(Key::Escape),
            RewriteColor::ChangeAll(Color::ORANGE),
            ctx,
        ))
        .margin(10),
        ManagedWidget::btn(Button::rectangle_svg(
            "assets/tools/settings.svg",
            "settings",
            None,
            RewriteColor::ChangeAll(Color::ORANGE),
            ctx,
        ))
        .margin(10),
    ];
    Composite::new(
        ezgui::Composite::new(ManagedWidget::row(row).bg(Color::grey(0.4)))
            .aligned(HorizontalAlignment::Left, VerticalAlignment::BottomAboveOSD)
            .build(ctx),
    )
    .cb(
        "settings",
        Box::new(|_, _| Some(Transition::Push(options::open_panel()))),
    )
}
