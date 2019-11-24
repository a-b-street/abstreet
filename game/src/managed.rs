use crate::game::{State, Transition};
use crate::ui::UI;
use ezgui::{layout, Color, EventCtx, GfxCtx, Line, MultiText, ScreenPt, Text, TextButton};

type Callback = Box<dyn Fn(&mut EventCtx, &mut UI) -> Option<Transition>>;

pub struct ManagedGUIStateBuilder {
    multi_txt: MultiText,
    buttons: Vec<(String, Callback)>,
}

impl ManagedGUIStateBuilder {
    pub fn draw_text(&mut self, txt: Text, pt: ScreenPt) {
        self.multi_txt.add(txt, pt);
    }

    pub fn text_button(&mut self, label: &str, onclick: Callback) {
        self.buttons.push((label.to_string(), onclick));
    }

    pub fn build(self, ctx: &EventCtx) -> Box<dyn State> {
        Box::new(ManagedGUIState {
            multi_txt: self.multi_txt,
            // TODO Default style. Lots of variations.
            buttons: self
                .buttons
                .into_iter()
                .map(|(label, onclick)| {
                    (
                        TextButton::new(
                            Text::from(Line(label).fg(Color::BLACK)),
                            Color::WHITE,
                            Color::ORANGE,
                            ctx,
                        ),
                        onclick,
                    )
                })
                .collect(),
        })
    }
}

pub struct ManagedGUIState {
    multi_txt: MultiText,
    buttons: Vec<(TextButton, Callback)>,
}

impl ManagedGUIState {
    pub fn builder() -> ManagedGUIStateBuilder {
        ManagedGUIStateBuilder {
            multi_txt: MultiText::new(),
            buttons: Vec::new(),
        }
    }
}

impl State for ManagedGUIState {
    fn event(&mut self, ctx: &mut EventCtx, ui: &mut UI) -> Transition {
        layout::stack_vertically(
            layout::ContainerOrientation::Centered,
            ctx,
            self.buttons
                .iter_mut()
                .map(|(btn, _)| btn as &mut dyn layout::Widget)
                .collect(),
        );
        for (btn, onclick) in self.buttons.iter_mut() {
            btn.event(ctx);
            if btn.clicked() {
                if let Some(t) = (onclick)(ctx, ui) {
                    return t;
                }
            }
        }
        Transition::Keep
    }

    fn draw_default_ui(&self) -> bool {
        false
    }

    fn draw(&self, g: &mut GfxCtx, ui: &UI) {
        // Happens to be a nice background color too ;)
        g.clear(ui.cs.get("grass"));
        self.multi_txt.draw(g);
        for (btn, _) in &self.buttons {
            btn.draw(g);
        }
    }
}
