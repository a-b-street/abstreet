use crate::{
    Btn, Choice, Color, Composite, Dropdown, EventCtx, GeomBatch, GfxCtx, JustDraw, MultiKey,
    ScreenDims, ScreenPt, Widget, WidgetImpl, WidgetOutput,
};
use geom::Polygon;

// TODO Radio buttons in the menu
pub struct PersistentSplit<T: Clone + PartialEq> {
    current_value: T,
    hotkey: Option<MultiKey>,
    label: String,
    composite: Composite,
}

impl<T: 'static + PartialEq + Clone + std::fmt::Debug> PersistentSplit<T> {
    pub fn new(
        ctx: &mut EventCtx,
        label: &str,
        default_value: T,
        hotkey: Option<MultiKey>,
        choices: Vec<Choice<T>>,
    ) -> Widget {
        let dropdown = Dropdown::new(ctx, "change", default_value.clone(), choices, true);
        let btn = Btn::plaintext(dropdown.current_value_label()).build(ctx, label, hotkey.clone());
        let spacer = JustDraw::wrap(
            ctx,
            GeomBatch::from(vec![(
                Color::WHITE.alpha(0.5),
                Polygon::rectangle(3.0, btn.widget.get_dims().height),
            )]),
        );
        let composite = Composite::new(Widget::custom_row(vec![
            btn,
            spacer,
            Widget::new(Box::new(dropdown)).named("dropdown"),
        ]))
        .build_custom(ctx);
        Widget::new(Box::new(PersistentSplit {
            composite,
            current_value: default_value,
            hotkey,
            label: label.to_string(),
        }))
        .named(label)
    }
}

impl<T: 'static + PartialEq + Clone> PersistentSplit<T> {
    pub fn current_value(&self) -> T {
        self.current_value.clone()
    }
}

impl<T: 'static + Clone + PartialEq> WidgetImpl for PersistentSplit<T> {
    fn get_dims(&self) -> ScreenDims {
        self.composite.get_dims()
    }

    fn set_pos(&mut self, top_left: ScreenPt) {
        self.composite.set_pos(top_left);
    }

    fn event(&mut self, ctx: &mut EventCtx, output: &mut WidgetOutput) {
        match self.composite.event(ctx) {
            Some(o) => {
                output.outcome = Some(o);
                return;
            }
            None => {}
        }

        let new_value = self.composite.dropdown_value("dropdown");
        if new_value != self.current_value {
            self.current_value = new_value;
            self.composite.replace(
                ctx,
                &self.label,
                Btn::plaintext(self.composite.dropdown_value_label::<T>("dropdown")).build(
                    ctx,
                    self.label.clone(),
                    self.hotkey.clone(),
                ),
            );
            output.redo_layout = true;
        }
    }

    fn draw(&self, g: &mut GfxCtx) {
        self.composite.draw(g);
    }
}
