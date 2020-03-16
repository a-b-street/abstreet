use crate::colors;
use crate::game::Transition;
use crate::managed::WrappedComposite;
use crate::options;
use ezgui::{
    hotkey, Btn, Composite, EventCtx, HorizontalAlignment, Key, ManagedWidget, RewriteColor,
    VerticalAlignment,
};

pub fn tool_panel(ctx: &mut EventCtx) -> WrappedComposite {
    let row = vec![
        // TODO Maybe this is confusing -- it doesn't jump to the title screen necessarily.
        // Caller has to handle this one
        Btn::svg(
            "../data/system/assets/tools/home.svg",
            RewriteColor::ChangeAll(colors::HOVERING),
        )
        .build(ctx, "back", hotkey(Key::Escape))
        .margin(10),
        Btn::svg(
            "../data/system/assets/tools/settings.svg",
            RewriteColor::ChangeAll(colors::HOVERING),
        )
        .build(ctx, "settings", None)
        .margin(10),
    ];
    WrappedComposite::new(
        Composite::new(ManagedWidget::row(row).bg(colors::PANEL_BG))
            .aligned(HorizontalAlignment::Left, VerticalAlignment::BottomAboveOSD)
            .build(ctx),
    )
    .cb(
        "settings",
        Box::new(|ctx, app| {
            Some(Transition::Push(Box::new(options::OptionsPanel::new(
                ctx, app,
            ))))
        }),
    )
}
