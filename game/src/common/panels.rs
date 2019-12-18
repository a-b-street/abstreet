use crate::common::{navigate, shortcuts};
use crate::edit::EditMode;
use crate::game::Transition;
use crate::managed::{Composite, ManagedWidget};
use crate::options;
use crate::sandbox::GameplayMode;
use crate::ui::UI;
use ezgui::{hotkey, lctrl, Button, Color, EventCtx, Key, Line, RewriteColor, ScreenPt, Text};

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

pub fn edit_map_panel(ctx: &EventCtx, ui: &UI, gameplay: GameplayMode) -> Composite {
    Composite::minimal_size(
        ManagedWidget::row(vec![
            ManagedWidget::col(vec![
                ManagedWidget::draw_text(ctx, Text::from(Line("Sandbox"))),
                ManagedWidget::draw_text(ctx, Text::from(Line(ui.primary.map.get_name()))),
            ]),
            ManagedWidget::col(vec![
                // TODO icon button
                ManagedWidget::text_button(
                    ctx,
                    "edit map",
                    lctrl(Key::E),
                    Box::new(move |ctx, ui| {
                        ui.primary.clear_sim();
                        Some(Transition::Replace(Box::new(EditMode::new(
                            ctx,
                            gameplay.clone(),
                        ))))
                    }),
                ),
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
        ScreenPt::new(ctx.canvas.window_width / 2.0, 5.0),
    )
}
