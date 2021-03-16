use crate::{
    Color, DrawWithTooltips, EventCtx, GeomBatch, JustDraw, RewriteColor, ScreenDims, ScreenPt,
    Text, Widget,
};
use geom::Bounds;

#[derive(Clone, Debug)]
pub struct Image<'a> {
    source: ImageSource<'a>,
    tooltip: Option<Text>,
    color: Option<RewriteColor>,
}

#[derive(Clone, Debug)]
pub enum ImageSource<'a> {
    Path(&'a str),
    Bytes { bytes: &'a [u8], cache_key: &'a str },
    GeomBatch(GeomBatch, geom::Bounds),
}

impl ImageSource<'_> {
    pub fn load(&self, prerender: &crate::Prerender) -> (GeomBatch, geom::Bounds) {
        use crate::svg;
        match self {
            ImageSource::Path(image_path) => svg::load_svg(prerender, image_path),
            ImageSource::Bytes { bytes, cache_key } => {
                svg::load_svg_bytes(prerender, cache_key, bytes).expect(&format!(
                    "Failed to load svg from bytes. cache_key: {}",
                    cache_key
                ))
            }
            ImageSource::GeomBatch(geom_batch, bounds) => (geom_batch.clone(), *bounds),
        }
    }
}

impl<'a> Image<'a> {
    /// An SVG image, read from `filename`, which is colored to match Style.icon_fg
    pub fn icon(filename: &'a str) -> Self {
        Self {
            source: ImageSource::Path(filename),
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

    pub fn bytes(labeled_bytes: (&'a str, &'a [u8])) -> Self {
        Self {
            source: ImageSource::Bytes {
                cache_key: labeled_bytes.0,
                bytes: labeled_bytes.1,
            },
            tooltip: None,
            color: None,
        }
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

    pub fn batch(&self, ctx: &EventCtx) -> (GeomBatch, Bounds) {
        let (mut batch, bounds) = self.source.load(&ctx.prerender);

        let color = self
            .color
            .unwrap_or(RewriteColor::ChangeAll(ctx.style.icon_fg));
        batch = batch.color(color);

        // Preserve the padding in the SVG.
        // TODO Maybe always do this, add a way to autocrop() to remove it if needed.
        batch.push(Color::CLEAR, bounds.get_rectangle());

        (batch, bounds)
    }

    pub fn into_widget(self, ctx: &EventCtx) -> Widget {
        let (batch, bounds) = self.batch(ctx);

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
