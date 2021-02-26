use crate::{
    Color, DrawWithTooltips, EventCtx, GeomBatch, JustDraw, RewriteColor, ScreenDims, ScreenPt,
    Text, Widget,
};

pub struct Image<'a> {
    filename: &'a str,
    tooltip: Option<Text>,
    color: Option<RewriteColor>,
}

impl<'a> Image<'a> {
    /// An SVG image, read from `filename`, which is colored to match Style.icon_fg
    pub fn icon(filename: &'a str) -> Self {
        Self {
            filename,
            tooltip: None,
            color: None,
        }
    }

    /// An SVG image, read from `filename`.
    ///
    /// The image's intrinsic colors will be used, it will not be tinted like `Image::icon`, unless
    /// you call `color()`
    pub fn untinted(filename: &'a str) -> Self {
        Self::icon(filename).color(RewriteColor::NoOp)
    }

    /// Add a tooltip to appear when hovering over the image.
    pub fn tooltip(mut self, tooltip: Text) -> Self {
        self.tooltip = Some(tooltip);
        self
    }

    /// Transform the color of the image.
    pub fn color<RWC: Into<RewriteColor>>(mut self, color: RWC) -> Self {
        self.color = Some(color.into());
        self
    }

    pub fn into_widget(self, ctx: &EventCtx) -> Widget {
        // TODO: consolidate the impl from widgetry::widgets::button::Image which allows other
        // sources of images, like bytes and a raw GeomBatch.

        let (mut batch, bounds) = crate::svg::load_svg(ctx.prerender, self.filename);

        let color = self
            .color
            .unwrap_or(RewriteColor::ChangeAll(ctx.style.icon_fg));
        batch = batch.color(color);

        // Preserve the padding in the SVG.
        // TODO Maybe always do this, add a way to autocrop() to remove it if needed.
        batch.push(Color::CLEAR, bounds.get_rectangle());

        if let Some(tooltip) = self.tooltip {
            DrawWithTooltips::new(
                ctx,
                batch,
                vec![(bounds.get_rectangle(), tooltip)],
                Box::new(|_| GeomBatch::new()),
            )
        } else {
            Widget::new(Box::new(JustDraw {
                dims: ScreenDims::new(bounds.width(), bounds.height()),
                draw: ctx.upload(batch),
                top_left: ScreenPt::new(0.0, 0.0),
            }))
        }
    }
}
