use crate::colors;
use crate::game::Transition;
use crate::managed::WrappedComposite;
use crate::options;
use ezgui::{
    hotkey, Button, Composite, EventCtx, HorizontalAlignment, Key, ManagedWidget, RewriteColor,
    VerticalAlignment,
};

pub fn tool_panel(ctx: &mut EventCtx) -> WrappedComposite {
    let row = vec![
        // TODO Maybe this is confusing -- it doesn't jump to the title screen necessarily.
        // Caller has to handle this one
        ManagedWidget::btn(Button::rectangle_svg(
            "../data/system/assets/tools/home.svg",
            "back",
            hotkey(Key::Escape),
            RewriteColor::ChangeAll(colors::HOVERING),
            ctx,
        ))
        .margin(10),
        ManagedWidget::btn(Button::rectangle_svg(
            "../data/system/assets/tools/settings.svg",
            "settings",
            None,
            RewriteColor::ChangeAll(colors::HOVERING),
            ctx,
        ))
        .margin(10),
    ];
    WrappedComposite::new(
        Composite::new(ManagedWidget::row(row).bg(colors::PANEL_BG))
            .aligned(HorizontalAlignment::Left, VerticalAlignment::BottomAboveOSD)
            .build(ctx),
    )
    .cb(
        "settings",
        Box::new(|_, _| Some(Transition::Push(options::open_panel()))),
    )
}
