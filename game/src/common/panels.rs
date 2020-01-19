use crate::edit::EditMode;
use crate::game::Transition;
use crate::managed::Composite;
use crate::options;
use crate::sandbox::GameplayMode;
use ezgui::{
    hotkey, lctrl, Button, Color, EventCtx, HorizontalAlignment, Key, ManagedWidget, RewriteColor,
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

pub fn edit_map_panel(ctx: &mut EventCtx, gameplay: GameplayMode) -> Composite {
    Composite::new(
        ezgui::Composite::new(
            Composite::svg_button(ctx, "assets/tools/edit_map.svg", "edit map", lctrl(Key::E))
                .bg(Color::grey(0.4)),
        )
        .aligned(HorizontalAlignment::Center, VerticalAlignment::Top)
        .build(ctx),
    )
    .cb(
        "edit map",
        Box::new(move |ctx, ui| {
            Some(Transition::Replace(Box::new(EditMode::new(
                ctx,
                ui,
                gameplay.clone(),
            ))))
        }),
    )
}
