use ezgui::EventCtx;

pub struct PerObjectActions {
    pub click_action: Option<String>,
}

impl PerObjectActions {
    pub fn new() -> PerObjectActions {
        PerObjectActions { click_action: None }
    }

    pub fn reset(&mut self) {
        self.click_action = None;
    }

    pub fn left_click<S: Into<String>>(&mut self, ctx: &mut EventCtx, label: S) -> bool {
        assert!(self.click_action.is_none());
        self.click_action = Some(label.into());
        ctx.normal_left_click()
    }
}
