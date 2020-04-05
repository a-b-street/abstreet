use crate::{
    Btn, Button, Choice, Color, Dropdown, EventCtx, GeomBatch, GfxCtx, JustDraw, MultiKey, Outcome,
    ScreenDims, ScreenPt, Widget, WidgetImpl,
};
use geom::Polygon;

// TODO Radio buttons in the menu
pub struct PersistentSplit<T: Clone + PartialEq> {
    current_value: T,
    btn: Button,
    spacer: JustDraw,
    dropdown: Dropdown<T>,
}

impl<T: 'static + PartialEq + Clone> PersistentSplit<T> {
    pub fn new(
        ctx: &EventCtx,
        label: &str,
        default_value: T,
        hotkey: Option<MultiKey>,
        choices: Vec<Choice<T>>,
    ) -> Widget {
        let dropdown = Dropdown::new(ctx, "change", default_value, choices, true);
        let btn = Btn::plaintext(dropdown.current_value_label())
            .build(ctx, label, hotkey)
            .take_btn();

        Widget::new(Box::new(PersistentSplit {
            current_value: dropdown.current_value(),
            spacer: JustDraw::wrap(
                ctx,
                GeomBatch::from(vec![(
                    Color::WHITE.alpha(0.5),
                    Polygon::rectangle(3.0, btn.get_dims().height),
                )]),
            )
            .take_just_draw(),
            btn,
            dropdown,
        }))
        .named(label)
    }

    pub fn current_value(&self) -> T {
        self.current_value.clone()
    }
}

impl<T: 'static + Clone + PartialEq> WidgetImpl for PersistentSplit<T> {
    fn get_dims(&self) -> ScreenDims {
        let dims1 = self.btn.get_dims();
        let dims2 = self.spacer.get_dims();
        let dims3 = self.dropdown.get_dims();
        ScreenDims::new(
            dims1.width + dims2.width + dims3.width,
            dims1.height.max(dims2.height).max(dims3.height),
        )
    }

    fn set_pos(&mut self, top_left: ScreenPt) {
        self.btn.set_pos(top_left);
        self.spacer
            .set_pos(ScreenPt::new(top_left.x + self.btn.dims.width, top_left.y));
        self.dropdown.set_pos(ScreenPt::new(
            top_left.x + self.btn.dims.width + self.spacer.get_dims().width,
            top_left.y,
        ));
    }

    fn event(&mut self, ctx: &mut EventCtx, redo_layout: &mut bool) -> Option<Outcome> {
        if let Some(o) = self.btn.event(ctx, redo_layout) {
            return Some(o);
        }

        self.dropdown.event(ctx, redo_layout);
        let new_value = self.dropdown.current_value();
        if new_value != self.current_value {
            self.current_value = new_value;
            let hotkey = self.btn.hotkey.take();
            let label = self.btn.action.clone();
            self.btn = Btn::plaintext(self.dropdown.current_value_label())
                .build(ctx, label, hotkey)
                .take_btn();
            *redo_layout = true;
        }

        None
    }

    fn draw(&self, g: &mut GfxCtx) {
        self.btn.draw(g);
        self.spacer.draw(g);
        self.dropdown.draw(g);
    }
}
