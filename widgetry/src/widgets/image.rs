use crate::{
    Color, ContentMode, CornerRounding, DrawWithTooltips, EdgeInsets, EventCtx, GeomBatch,
    JustDraw, RewriteColor, ScreenDims, ScreenPt, Text, Widget,
};
use geom::{Bounds, Polygon, Pt2D};

use std::borrow::Cow;

/// A stylable UI component builder which presents vector graphics from an [`ImageSource`].
#[derive(Clone, Debug, Default)]
pub struct Image<'a, 'c> {
    source: Option<Cow<'c, ImageSource<'a>>>,
    tooltip: Option<Text>,
    color: Option<RewriteColor>,
    content_mode: Option<ContentMode>,
    corner_rounding: Option<CornerRounding>,
    padding: Option<EdgeInsets>,
    bg_color: Option<Color>,
    dims: Option<ScreenDims>,
}

/// The visual
#[derive(Clone, Debug)]
pub enum ImageSource<'a> {
    /// Path to an SVG file
    Path(&'a str),

    /// UTF-8 encoded bytes of an SVG
    Bytes { bytes: &'a [u8], cache_key: &'a str },

    /// Previously rendered graphics, in the form of a [`GeomBatch`], can
    /// be packaged as an `Image`.
    GeomBatch(GeomBatch, geom::Bounds),
}

impl ImageSource<'_> {
    /// Process `self` into a [`GeomBatch`].
    ///
    /// The underlying implementation makes use of caching to avoid re-parsing SVGs.
    pub fn load(&self, prerender: &crate::Prerender) -> (GeomBatch, geom::Bounds) {
        use crate::svg;
        match self {
            ImageSource::Path(image_path) => svg::load_svg(prerender, image_path),
            ImageSource::Bytes { bytes, cache_key } => {
                svg::load_svg_bytes(prerender, cache_key, bytes).unwrap_or_else(
                    |_| panic!("Failed to load svg from bytes. cache_key: {}", cache_key))
            }
            ImageSource::GeomBatch(geom_batch, bounds) => (geom_batch.clone(), *bounds),
        }
    }
}

impl<'a, 'c> Image<'a, 'c> {
    /// An `Image` with no renderable content. Useful for starting a template for creating
    /// several similar images using a builder pattern.
    pub fn empty() -> Self {
        Self {
            ..Default::default()
        }
    }

    /// Create an SVG `Image`, read from `filename`, which is colored to match `Style.icon_fg`
    pub fn from_path(filename: &'a str) -> Self {
        Self {
            source: Some(Cow::Owned(ImageSource::Path(filename))),
            ..Default::default()
        }
    }

    /// Create an SVG `Image`, read from `filename`.
    ///
    /// The image's intrinsic colors will be used, it will not be tinted like `Image::icon`, unless
    /// you also call `color()`
    pub fn untinted(filename: &'a str) -> Self {
        Self::from_path(filename).color(RewriteColor::NoOp)
    }

    /// Create a new SVG `Image` from bytes.
    ///
    /// * `labeled_bytes`: is a (`label`, `bytes`) tuple you can generate with
    ///   [`include_labeled_bytes!`]
    /// * `label`: a label to describe the bytes for debugging purposes
    /// * `bytes`: UTF-8 encoded bytes of the SVG
    pub fn from_bytes(labeled_bytes: (&'a str, &'a [u8])) -> Self {
        Self {
            source: Some(Cow::Owned(ImageSource::Bytes {
                cache_key: labeled_bytes.0,
                bytes: labeled_bytes.1,
            })),
            ..Default::default()
        }
    }

    /// Create a new `Image` from a [`GeomBatch`].
    pub fn from_batch(batch: GeomBatch, bounds: Bounds) -> Self {
        Self {
            source: Some(Cow::Owned(ImageSource::GeomBatch(batch, bounds))),
            ..Default::default()
        }
    }

    /// Set a new source for the `Image`'s data.
    ///
    /// This will replace any previously set source.
    pub fn source(mut self, source: ImageSource<'a>) -> Self {
        self.source = Some(Cow::Owned(source));
        self
    }

    /// Set the path to an SVG file for the image.
    ///
    /// This will replace any image source previously set.
    pub fn source_path(self, path: &'a str) -> Self {
        self.source(ImageSource::Path(path))
    }

    /// Set the bytes for the image.
    ///
    /// This will replace any image source previously set.
    ///
    /// * `labeled_bytes`: is a (`label`, `bytes`) tuple you can generate with
    ///   [`include_labeled_bytes!`]
    /// * `label`: a label to describe the bytes for debugging purposes
    /// * `bytes`: UTF-8 encoded bytes of the SVG
    pub fn source_bytes(self, labeled_bytes: (&'a str, &'a [u8])) -> Self {
        let (label, bytes) = labeled_bytes;
        self.source(ImageSource::Bytes {
            bytes,
            cache_key: label,
        })
    }

    /// Set the GeomBatch for the button.
    ///
    /// This will replace any image source previously set.
    ///
    /// This method is useful when doing more complex transforms. For example, to re-write more than
    /// one color for your image, do so externally and pass in the resultant GeomBatch here.
    pub fn source_batch(self, batch: GeomBatch, bounds: geom::Bounds) -> Self {
        self.source(ImageSource::GeomBatch(batch, bounds))
    }

    /// Add a tooltip to appear when hovering over the image.
    pub fn tooltip(mut self, tooltip: impl Into<Text>) -> Self {
        self.tooltip = Some(tooltip.into());
        self
    }

    /// Create a new `Image` based on `self`, but overriding with any values set on `other`.
    pub fn merged_image_style(&'c self, other: &'c Self) -> Self {
        #![allow(clippy::or_fun_call)]
        let source_cow: Option<&Cow<'c, ImageSource>> =
            other.source.as_ref().or(self.source.as_ref());
        let source: Option<Cow<'c, ImageSource>> = source_cow.map(|source: &Cow<ImageSource>| {
            let source: &ImageSource = source;
            Cow::Borrowed(source)
        });

        Self {
            source,
            // PERF: we could make tooltip a cow to eliminate clone
            tooltip: other.tooltip.clone().or(self.tooltip.clone()),
            color: other.color.or(self.color),
            content_mode: other.content_mode.or(self.content_mode),
            corner_rounding: other.corner_rounding.or(self.corner_rounding),
            padding: other.padding.or(self.padding),
            bg_color: other.bg_color.or(self.bg_color),
            dims: other.dims.or(self.dims),
        }
    }

    /// Rewrite the color of the image.
    pub fn color<RWC: Into<RewriteColor>>(mut self, value: RWC) -> Self {
        self.color = Some(value.into());
        self
    }

    /// Set a background color for the image.
    pub fn bg_color(mut self, value: Color) -> Self {
        self.bg_color = Some(value);
        self
    }

    /// Scale the bounds containing the image. If `image_dims` are not specified, the images
    /// intrinsic size will be used.
    ///
    /// See [`Self::content_mode`] to control how the image scales to fit
    /// its custom bounds.
    pub fn dims<D: Into<ScreenDims>>(mut self, dims: D) -> Self {
        self.dims = Some(dims.into());
        self
    }

    /// If a custom `dims` was set, control how the image should be scaled to its new bounds
    ///
    /// If `dims` were not specified, the image will not be scaled, so content_mode has no
    /// affect.
    ///
    /// The default, [`ContentMode::ScaleAspectFit`] will only grow as much as it can while
    /// maintaining its aspect ratio and not exceeding its bounds
    pub fn content_mode(mut self, value: ContentMode) -> Self {
        self.content_mode = Some(value);
        self
    }

    /// Set independent rounding for each of the image's corners
    pub fn corner_rounding<R: Into<CornerRounding>>(mut self, value: R) -> Self {
        self.corner_rounding = Some(value.into());
        self
    }

    /// Set padding for the image
    pub fn padding<EI: Into<EdgeInsets>>(mut self, value: EI) -> Self {
        self.padding = Some(value.into());
        self
    }

    /// Render the `Image` and any styling (padding, background, etc.) to a `GeomBatch`.
    pub fn build_batch(&self, ctx: &EventCtx) -> Option<(GeomBatch, Bounds)> {
        #![allow(clippy::or_fun_call)]
        self.source.as_ref().map(|source| {
            let (mut image_batch, image_bounds) = source.load(ctx.prerender);

            image_batch = image_batch.color(
                self.color
                    .unwrap_or(RewriteColor::ChangeAll(ctx.style().icon_fg)),
            );

            match self.dims {
                None => {
                    // Preserve any padding intrinsic to the SVG.
                    image_batch.push(Color::CLEAR, image_bounds.get_rectangle());
                    (image_batch, image_bounds)
                }
                Some(image_dims) => {
                    if image_bounds.width() != 0.0 && image_bounds.height() != 0.0 {
                        let (x_factor, y_factor) = (
                            image_dims.width / image_bounds.width(),
                            image_dims.height / image_bounds.height(),
                        );
                        image_batch = match self.content_mode.unwrap_or_default() {
                            ContentMode::ScaleToFill => image_batch.scale_xy(x_factor, y_factor),
                            ContentMode::ScaleAspectFit => {
                                image_batch.scale(x_factor.min(y_factor))
                            }
                            ContentMode::ScaleAspectFill => {
                                image_batch.scale(x_factor.max(y_factor))
                            }
                        };
                    }

                    let image_corners = self.corner_rounding.unwrap_or_default();
                    let padding = self.padding.unwrap_or_default();

                    let mut container_batch = GeomBatch::new();
                    let container_bounds = Bounds {
                        min_x: 0.0,
                        min_y: 0.0,
                        max_x: image_dims.width + padding.left + padding.right,
                        max_y: image_dims.height + padding.top + padding.bottom,
                    };
                    let container = match image_corners {
                        CornerRounding::FullyRounded => {
                            Polygon::pill(container_bounds.width(), container_bounds.height())
                        }
                        CornerRounding::CornerRadii(image_corners) => Polygon::rounded_rectangle(
                            container_bounds.width(),
                            container_bounds.height(),
                            image_corners,
                        ),
                    };

                    let image_bg = self.bg_color.unwrap_or(Color::CLEAR);
                    container_batch.push(image_bg, container);

                    let center = Pt2D::new(
                        image_dims.width / 2.0 + padding.left,
                        image_dims.height / 2.0 + padding.top,
                    );
                    image_batch = image_batch.autocrop().centered_on(center);
                    container_batch.append(image_batch);

                    (container_batch, container_bounds)
                }
            }
        })
    }

    pub fn into_widget(self, ctx: &EventCtx) -> Widget {
        match self.build_batch(ctx) {
            None => Widget::nothing(),
            Some((batch, bounds)) => {
                if let Some(tooltip) = self.tooltip {
                    DrawWithTooltips::new_widget(
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
    }
}
