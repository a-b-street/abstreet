use crate::app::App;
use crate::game::Transition;
use crate::managed::WrappedComposite;
use crate::options;
use ezgui::{
    hotkey, Btn, Composite, EventCtx, HorizontalAlignment, Key, VerticalAlignment, Widget,
};

pub fn tool_panel(ctx: &mut EventCtx, app: &App) -> WrappedComposite {
    let row = vec![
        // TODO Maybe this is confusing -- it doesn't jump to the title screen necessarily.
        // Caller has to handle this one
        Btn::svg_def("../data/system/assets/tools/home.svg")
            .build(ctx, "back", hotkey(Key::Escape))
            .margin(10),
        Btn::svg_def("../data/system/assets/tools/settings.svg")
            .build(ctx, "settings", None)
            .margin(10),
    ];
    WrappedComposite::new(
        Composite::new(Widget::row(row).bg(app.cs.panel_bg))
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
