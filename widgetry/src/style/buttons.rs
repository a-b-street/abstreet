use geom::CornerRadii;

use super::ButtonTheme;
use crate::{
    include_labeled_bytes, ButtonBuilder, ControlState, EventCtx, Key, ScreenDims, Style, Widget,
};

pub trait StyledButtons<'a> {
    // Everything above this is deprecated and should be replaced with a call to a method
    // which chooses a light vs. dark button as is appropriate for the theme. However, we can't
    // "just" do this without breaking some current layouts.
    fn btn_solid_panel(&self) -> ButtonBuilder<'a>;
    fn btn_solid_floating(&self) -> ButtonBuilder<'a>;

    fn btn_plain(&self) -> ButtonBuilder<'a>;
    fn btn_plain_text(&self, text: &'a str) -> ButtonBuilder<'a> {
        self.btn_plain().label_text(text)
    }
    fn btn_plain_icon(&self, image_path: &'a str) -> ButtonBuilder<'a> {
        icon_button(self.btn_plain().image_path(image_path))
    }
    fn btn_plain_icon_bytes(&self, labeled_bytes: (&'a str, &'a [u8])) -> ButtonBuilder<'a> {
        icon_button(self.btn_plain().image_bytes(labeled_bytes))
    }
    fn btn_plain_icon_text(&self, image_path: &'a str, text: &'a str) -> ButtonBuilder<'a> {
        self.btn_plain()
            .label_text(text)
            .image_path(image_path)
            .image_dims(ScreenDims::square(18.0))
    }

    fn btn_outline(&self) -> ButtonBuilder<'a>;
    fn btn_outline_text(&self, text: &'a str) -> ButtonBuilder<'a> {
        self.btn_outline().label_text(text)
    }
    fn btn_outline_icon(&self, image_path: &'a str) -> ButtonBuilder<'a> {
        icon_button(self.btn_outline().image_path(image_path))
    }
    fn btn_outline_icon_text(&self, image_path: &'a str, text: &'a str) -> ButtonBuilder<'a> {
        self.btn_outline()
            .label_text(text)
            .image_path(image_path)
            .image_dims(ScreenDims::square(18.0))
    }

    fn btn_solid(&self) -> ButtonBuilder<'a>;
    fn btn_solid_text(&self, text: &'a str) -> ButtonBuilder<'a> {
        self.btn_solid().label_text(text)
    }
    fn btn_solid_icon(&self, image_path: &'a str) -> ButtonBuilder<'a> {
        icon_button(self.btn_solid().image_path(image_path))
    }
    fn btn_solid_icon_bytes(&self, labeled_bytes: (&'a str, &'a [u8])) -> ButtonBuilder<'a> {
        icon_button(self.btn_solid().image_bytes(labeled_bytes))
    }
    fn btn_solid_icon_text(&self, image_path: &'a str, text: &'a str) -> ButtonBuilder<'a> {
        self.btn_solid()
            .label_text(text)
            .image_path(image_path)
            .image_dims(ScreenDims::square(18.0))
    }

    fn btn_plain_destructive(&self) -> ButtonBuilder<'a>;
    fn btn_plain_destructive_text(&self, text: &'a str) -> ButtonBuilder<'a> {
        self.btn_plain_destructive().label_text(text)
    }
    fn btn_plain_destructive_icon(&self, image_path: &'a str) -> ButtonBuilder<'a> {
        icon_button(self.btn_plain_destructive().image_path(image_path))
    }

    fn btn_solid_destructive(&self) -> ButtonBuilder<'a>;
    fn btn_solid_destructive_text(&self, text: &'a str) -> ButtonBuilder<'a> {
        self.btn_solid_destructive().label_text(text)
    }
    fn btn_solid_destructive_icon(&self, image_path: &'a str) -> ButtonBuilder<'a> {
        icon_button(self.btn_solid_destructive().image_path(image_path))
    }
    fn btn_solid_destructive_icon_text(
        &self,
        image_path: &'a str,
        text: &'a str,
    ) -> ButtonBuilder<'a> {
        self.btn_solid_destructive()
            .label_text(text)
            .image_path(image_path)
            .image_dims(ScreenDims::square(18.0))
    }

    fn btn_outline_destructive(&self) -> ButtonBuilder<'a>;
    fn btn_outline_destructive_text(&self, text: &'a str) -> ButtonBuilder<'a> {
        self.btn_outline_destructive().label_text(text)
    }
    fn btn_outline_destructive_icon(&self, image_path: &'a str) -> ButtonBuilder<'a> {
        icon_button(self.btn_outline_destructive().image_path(image_path))
    }
    fn btn_outline_destructive_icon_text(
        &self,
        image_path: &'a str,
        text: &'a str,
    ) -> ButtonBuilder<'a> {
        self.btn_outline_destructive()
            .label_text(text)
            .image_path(image_path)
            .image_dims(ScreenDims::square(18.0))
    }

    // Specific UI Elements

    /// title: name of previous screen, which you'll return to
    fn btn_back(&self, title: &'a str) -> ButtonBuilder<'a> {
        back_button(self.btn_plain(), title)
    }

    fn btn_outline_dropdown(&self) -> ButtonBuilder<'a> {
        dropdown_button(self.btn_outline())
    }

    fn btn_solid_dropdown(&self) -> ButtonBuilder<'a> {
        dropdown_button(self.btn_solid())
    }

    fn btn_outline_popup(&self, text: &'a str) -> ButtonBuilder<'a> {
        self.btn_outline_dropdown().label_text(text)
    }

    /// A right facing caret, like ">", suitable for paging to the "next" set of results
    fn btn_next(&self) -> ButtonBuilder<'a> {
        self.btn_plain_icon_bytes(include_labeled_bytes!("../../icons/next.svg"))
    }

    /// A left facing caret, like "<", suitable for paging to the "next" set of results
    fn btn_prev(&self) -> ButtonBuilder<'a> {
        self.btn_plain_icon_bytes(include_labeled_bytes!("../../icons/prev.svg"))
    }

    /// An "X" button to close the current state. Bound to the escape key.
    fn btn_close(&self) -> ButtonBuilder<'a> {
        self.btn_plain_icon_bytes(include_labeled_bytes!("../../icons/close.svg"))
            .hotkey(Key::Escape)
    }

    /// An "X" button to close the current state. Bound to the escape key and aligned to the right,
    /// usually after a title.
    fn btn_close_widget(&self, ctx: &EventCtx) -> Widget {
        self.btn_close().build_widget(ctx, "close").align_right()
    }
}

impl<'a> StyledButtons<'a> for Style {
    fn btn_solid_panel(&self) -> ButtonBuilder<'a> {
        self.btn_solid(&self.btn_solid_panel)
    }

    fn btn_solid_floating(&self) -> ButtonBuilder<'a> {
        self.btn_solid(&self.btn_solid_floating)
    }

    fn btn_solid(&self) -> ButtonBuilder<'a> {
        self.btn_solid(&self.btn_solid)
    }

    fn btn_outline(&self) -> ButtonBuilder<'a> {
        self.btn_outline(&self.btn_outline)
    }

    fn btn_plain(&self) -> ButtonBuilder<'a> {
        plain_button(&self.btn_outline)
    }

    fn btn_plain_destructive(&self) -> ButtonBuilder<'a> {
        plain_button(&self.btn_outline_destructive)
    }

    fn btn_solid_destructive(&self) -> ButtonBuilder<'a> {
        self.btn_solid(&self.btn_solid_destructive)
    }

    fn btn_outline_destructive(&self) -> ButtonBuilder<'a> {
        self.btn_outline(&self.btn_solid_destructive)
    }
}

impl<'a> Style {
    pub fn btn_solid(&self, button_style: &ButtonTheme) -> ButtonBuilder<'a> {
        plain_button(button_style).outline(
            self.outline_thickness,
            button_style.outline,
            ControlState::Default,
        )
    }

    pub fn btn_outline(&self, button_style: &ButtonTheme) -> ButtonBuilder<'a> {
        plain_button(button_style).outline(
            self.outline_thickness,
            button_style.outline,
            ControlState::Default,
        )
    }

    pub fn btn_light_popup_icon_text(
        &self,
        icon_path: &'a str,
        text: &'a str,
    ) -> ButtonBuilder<'a> {
        let outline_style = &self.btn_outline;
        let solid_style = &self.btn_solid;

        // The text is styled like an "outline" button, while the image is styled like a "solid"
        // button.
        self.btn_outline(outline_style)
            .label_text(text)
            .image_path(icon_path)
            .image_dims(25.0)
            .image_color(solid_style.fg, ControlState::Default)
            .image_bg_color(solid_style.bg, ControlState::Default)
            .image_bg_color(solid_style.bg_hover, ControlState::Hovered)
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
fn icon_button<'a>(builder: ButtonBuilder<'a>) -> ButtonBuilder<'a> {
    builder.padding(8.0).image_dims(25.0)
}

fn back_button<'a>(builder: ButtonBuilder<'a>, title: &'a str) -> ButtonBuilder<'a> {
    // DESIGN REVIEW: this button seems absurdly large
    builder
        .image_bytes(include_labeled_bytes!("../../icons/nav_back.svg"))
        .label_text(title)
        .padding_left(8.0)
        .font_size(30)
}

fn dropdown_button<'a>(builder: ButtonBuilder<'a>) -> ButtonBuilder<'a> {
    builder
        .image_bytes(include_labeled_bytes!("../../icons/arrow_drop_down.svg"))
        .image_dims(12.0)
        .stack_spacing(12.0)
        .label_first()
}

pub fn plain_button<'a>(button_style: &ButtonTheme) -> ButtonBuilder<'a> {
    ButtonBuilder::new()
        .label_color(button_style.fg, ControlState::Default)
        .label_color(button_style.fg_disabled, ControlState::Disabled)
        .image_color(button_style.fg, ControlState::Default)
        .image_color(button_style.fg_disabled, ControlState::Disabled)
        .bg_color(button_style.bg, ControlState::Default)
        .bg_color(button_style.bg_hover, ControlState::Hovered)
        .bg_color(button_style.bg_disabled, ControlState::Disabled)
}
