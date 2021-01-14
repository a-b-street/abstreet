use widgetry::{ButtonBuilder, ButtonState, Color};

pub struct Btn;

// TODO: make this a method on ColorScheme?
//       or introduce Theme which *has* a colorscheme
impl Btn {
    pub fn svg(path: &str, hover_bg_color: Color) -> ButtonBuilder {
        ButtonBuilder::new()
            .image_path(&path)
            .bg_color(hover_bg_color, ButtonState::Hover)
    }
}
