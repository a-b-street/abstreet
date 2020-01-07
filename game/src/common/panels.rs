use crate::common::{navigate, shortcuts, Warping};
use crate::edit::EditMode;
use crate::game::Transition;
use crate::managed::Composite;
use crate::options;
use crate::sandbox::GameplayMode;
use crate::ui::UI;
use ezgui::{
    hotkey, lctrl, Button, Color, EventCtx, EventLoopMode, HorizontalAlignment, Key, Line,
    ManagedWidget, RewriteColor, Text, VerticalAlignment,
};

// TODO Rethink this API.
pub fn tool_panel(ctx: &mut EventCtx, extra_buttons: Vec<ManagedWidget>) -> Composite {
    let mut row = vec![
        // TODO Maybe this is confusing -- it doesn't jump to the title screen necessarily.
        // Caller has to handle this one
        ManagedWidget::btn(Button::rectangle_svg(
            "assets/tools/home.svg",
            "back",
            hotkey(Key::Escape),
            RewriteColor::ChangeAll(Color::ORANGE),
            ctx,
        )),
        ManagedWidget::btn(Button::rectangle_svg(
            "assets/tools/settings.svg",
            "settings",
            None,
            RewriteColor::ChangeAll(Color::ORANGE),
            ctx,
        )),
        Composite::svg_button(ctx, "assets/tools/search.svg", "search", hotkey(Key::K)),
        Composite::svg_button(
            ctx,
            "assets/tools/shortcuts.svg",
            "shortcuts",
            hotkey(Key::SingleQuote),
        ),
        // TODO Mutex
        Composite::svg_button(
            ctx,
            "assets/minimap/zoom_out_fully.svg",
            "zoom out fully",
            None,
        ),
        Composite::svg_button(
            ctx,
            "assets/minimap/zoom_in_fully.svg",
            "zoom in fully",
            None,
        ),
    ];
    row.extend(extra_buttons);

    Composite::new(ezgui::Composite::aligned(
        ctx,
        (HorizontalAlignment::Left, VerticalAlignment::BottomAboveOSD),
        ManagedWidget::row(row.into_iter().map(|x| x.margin(10)).collect()).bg(Color::grey(0.4)),
    ))
    .cb(
        "settings",
        Box::new(|_, _| Some(Transition::Push(options::open_panel()))),
    )
    .cb(
        "search",
        Box::new(|_, ui| Some(Transition::Push(Box::new(navigate::Navigator::new(ui))))),
    )
    .cb(
        "shortcuts",
        Box::new(|_, _| Some(Transition::Push(shortcuts::ChoosingShortcut::new()))),
    )
    .cb(
        "zoom out fully",
        // TODO The zoom out level should show the full width/height -- that's kind of in minimap
        // code
        Box::new(|ctx, ui| {
            Some(Transition::PushWithMode(
                Warping::new(
                    ctx,
                    // TODO The animated zooming is too quick. Need to specify that we want to
                    // interpolate over the zoom factor.
                    ctx.canvas.center_to_map_pt().offset(1.0, 1.0),
                    Some(0.1),
                    None,
                    &mut ui.primary,
                ),
                EventLoopMode::Animation,
            ))
        }),
    )
    .cb(
        "zoom in fully",
        Box::new(|ctx, ui| {
            Some(Transition::PushWithMode(
                Warping::new(
                    ctx,
                    ctx.canvas.center_to_map_pt().offset(1.0, 1.0),
                    Some(10.0),
                    None,
                    &mut ui.primary,
                ),
                EventLoopMode::Animation,
            ))
        }),
    )
}

pub fn edit_map_panel(ctx: &mut EventCtx, ui: &UI, gameplay: GameplayMode) -> Composite {
    Composite::new(ezgui::Composite::aligned(
        ctx,
        (HorizontalAlignment::Center, VerticalAlignment::Top),
        ManagedWidget::row(vec![
            ManagedWidget::col(vec![
                ManagedWidget::draw_text(ctx, Text::from(Line("Sandbox"))),
                ManagedWidget::draw_text(ctx, Text::from(Line(ui.primary.map.get_name()))),
            ]),
            ManagedWidget::col(vec![
                // TODO icon button
                Composite::text_button(ctx, "edit map", lctrl(Key::E)),
                {
                    let edits = ui.primary.map.get_edits();
                    let mut txt = Text::from(Line(&edits.edits_name));
                    if edits.dirty {
                        txt.append(Line("*"));
                    }
                    ManagedWidget::draw_text(ctx, txt)
                },
            ]),
        ])
        .bg(Color::grey(0.4)),
    ))
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
