use geom::CornerRadii;

use crate::{
    include_labeled_bytes, ButtonBuilder, Color, ControlState, EventCtx, Key, OutlineStyle,
    ScreenDims, Style, Widget,
};

#[derive(Clone)]
pub struct ButtonStyle {
    pub fg: Color,
    pub fg_disabled: Color,
    pub outline: OutlineStyle,
    pub bg: Color,
    pub bg_hover: Color,
    pub bg_disabled: Color,
}

impl<'a, 'c> ButtonStyle {
    pub fn btn(&self) -> ButtonBuilder<'a, 'c> {
        let base = ButtonBuilder::new()
            .label_color(self.fg, ControlState::Default)
            .label_color(self.fg_disabled, ControlState::Disabled)
            .image_color(self.fg, ControlState::Default)
            .image_color(self.fg_disabled, ControlState::Disabled)
            .bg_color(self.bg, ControlState::Default)
            .bg_color(self.bg_hover, ControlState::Hovered)
            .bg_color(self.bg_disabled, ControlState::Disabled);

        if self.outline.0 > 0.0 {
            base.outline(self.outline, ControlState::Default)
        } else {
            base
        }
    }

    pub fn text<I: Into<String>>(&self, text: I) -> ButtonBuilder<'a, 'c> {
        self.btn().label_text(text)
    }

    pub fn icon(&self, image_path: &'a str) -> ButtonBuilder<'a, 'c> {
        icon_button(self.btn().image_path(image_path))
    }

    pub fn icon_bytes(&self, labeled_bytes: (&'a str, &'a [u8])) -> ButtonBuilder<'a, 'c> {
        icon_button(self.btn().image_bytes(labeled_bytes))
    }

    pub fn icon_text<I: Into<String>>(
        &self,
        image_path: &'a str,
        text: I,
    ) -> ButtonBuilder<'a, 'c> {
        self.btn()
            .label_text(text)
            .image_path(image_path)
            .image_dims(ScreenDims::square(18.0))
    }

    pub fn dropdown(&self) -> ButtonBuilder<'a, 'c> {
        self.icon_bytes(include_labeled_bytes!("../../icons/arrow_drop_down.svg"))
            .image_dims(12.0)
            .stack_spacing(12.0)
            .label_first()
    }

    pub fn popup(&self, text: &'a str) -> ButtonBuilder<'a, 'c> {
        self.dropdown().label_text(text)
    }
}

impl<'a, 'c> Style {
    /// title: name of previous screen, which you'll return to
    pub fn btn_back(&self, title: &'a str) -> ButtonBuilder<'a, 'c> {
        self.btn_plain
            .icon_bytes(include_labeled_bytes!("../../icons/nav_back.svg"))
            .label_text(title)
            .padding_left(8.0)
            .font_size(30)
    }

    /// A right facing caret, like ">", suitable for paging to the "next" set of results
    pub fn btn_next(&self) -> ButtonBuilder<'a, 'c> {
        self.btn_plain
            .icon_bytes(include_labeled_bytes!("../../icons/next.svg"))
    }

    /// A left facing caret, like "<", suitable for paging to the "next" set of results
    pub fn btn_prev(&self) -> ButtonBuilder<'a, 'c> {
        self.btn_plain
            .icon_bytes(include_labeled_bytes!("../../icons/prev.svg"))
    }

    /// An "X" button to close the current state. Bound to the escape key.
    pub fn btn_close(&self) -> ButtonBuilder<'a, 'c> {
        self.btn_plain
            .icon_bytes(include_labeled_bytes!("../../icons/close.svg"))
            .hotkey(Key::Escape)
    }

    /// An "X" button to close the current state. Bound to the escape key and aligned to the right,
    /// usually after a title.
    pub fn btn_close_widget(&self, ctx: &EventCtx) -> Widget {
        self.btn_close().build_widget(ctx, "close").align_right()
    }

    pub fn btn_popup_icon_text(&self, icon_path: &'a str, text: &'a str) -> ButtonBuilder<'a, 'c> {
        // The text is styled like an "outline" button, while the image is styled like a "solid"
        // button.
        self.btn_outline
            .btn()
            .label_text(text)
            .image_path(icon_path)
            .image_dims(25.0)
            .image_color(self.btn_solid.fg, ControlState::Default)
            .image_bg_color(self.btn_solid.bg, ControlState::Default)
            .image_bg_color(self.btn_solid.bg_hover, ControlState::Hovered)
            // Move the padding from the *entire button* to just the image, so we get a colored
            // padded area around the image.
            .padding(0)
            .image_padding(8.0)
            // ...though we still need to pad between the text and button edge
            .padding_right(8.0)
            // Round the button's image's exterior corners so they don't protrude past the button's
            // corners. However, per design, we want the images interior corners to be
            // unrounded.
            .image_corner_rounding(CornerRadii {
                top_left: 2.0,
                top_right: 0.0,
                bottom_right: 0.0,
                bottom_left: 2.0,
            })
    }
}

// Captures some constants for uniform styling of icon-only buttons
fn icon_button<'a, 'c>(builder: ButtonBuilder<'a, 'c>) -> ButtonBuilder<'a, 'c> {
    builder.padding(8.0).image_dims(25.0)
}
