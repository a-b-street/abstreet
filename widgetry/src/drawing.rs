use std::cell::{Cell, RefCell};

use geom::{Bounds, Polygon, Pt2D};

use crate::assets::Assets;
use crate::backend::{GfxCtxInnards, PrerenderInnards};
use crate::{
    Canvas, Color, Drawable, EventCtx, GeomBatch, Key, ScreenDims, ScreenPt, ScreenRectangle,
    Style, Text,
};

// We organize major layers of the app with whole number z values, with lower values being more on
// top.
//
// Within each layer, we must only adjust the z-offset of individual polygons within (-1, 0] to
// avoid traversing layers.
pub(crate) const MAPSPACE_Z: f32 = 1.0;
pub(crate) const SCREENSPACE_Z: f32 = 0.0;
pub(crate) const MENU_Z: f32 = -1.0;
pub(crate) const TOOLTIP_Z: f32 = -2.0;

#[derive(Debug)]
pub struct Uniforms {
    // (cam_x, cam_y, cam_zoom)
    pub transform: [f32; 3],
    // (window_width, window_height, Z values)
    pub window: [f32; 3],
}

impl Uniforms {
    pub fn new(canvas: &Canvas) -> Uniforms {
        Uniforms {
            transform: [
                canvas.cam_x as f32,
                canvas.cam_y as f32,
                canvas.cam_zoom as f32,
            ],
            window: [
                canvas.window_width as f32,
                canvas.window_height as f32,
                MAPSPACE_Z,
            ],
        }
    }
}

pub struct GfxCtx<'a> {
    pub(crate) inner: GfxCtxInnards<'a>,
    uniforms: Uniforms,

    screencap_mode: bool,
    pub(crate) naming_hint: Option<String>,

    // TODO Don't be pub. Delegate everything.
    pub canvas: &'a Canvas,
    pub prerender: &'a Prerender,
    style: &'a Style,

    pub(crate) num_draw_calls: usize,
    pub(crate) num_forks: usize,
}

impl<'a> GfxCtx<'a> {
    pub(crate) fn new(
        prerender: &'a Prerender,
        canvas: &'a Canvas,
        style: &'a Style,
        screencap_mode: bool,
    ) -> GfxCtx<'a> {
        let uniforms = Uniforms::new(canvas);
        GfxCtx {
            inner: prerender.inner.draw_new_frame(),
            uniforms,
            canvas,
            style,
            prerender,
            num_draw_calls: 0,
            num_forks: 0,
            screencap_mode,
            naming_hint: None,
        }
    }

    // Up to the caller to call unfork()!
    // TODO Canvas doesn't understand this change, so things like text drawing that use
    // map_to_screen will just be confusing.
    pub fn fork(
        &mut self,
        top_left_map: Pt2D,
        top_left_screen: ScreenPt,
        zoom: f64,
        z: Option<f32>,
    ) {
        // map_to_screen of top_left_map should be top_left_screen
        let cam_x = (top_left_map.x() * zoom) - top_left_screen.x;
        let cam_y = (top_left_map.y() * zoom) - top_left_screen.y;

        self.uniforms.transform = [cam_x as f32, cam_y as f32, zoom as f32];
        self.uniforms.window = [
            self.canvas.window_width as f32,
            self.canvas.window_height as f32,
            z.unwrap_or(SCREENSPACE_Z),
        ];
        self.num_forks += 1;
    }

    pub fn fork_screenspace(&mut self) {
        self.uniforms.transform = [0.0, 0.0, 1.0];
        self.uniforms.window = [
            self.canvas.window_width as f32,
            self.canvas.window_height as f32,
            SCREENSPACE_Z,
        ];
        self.num_forks += 1;
    }

    pub fn unfork(&mut self) {
        self.uniforms = Uniforms::new(&self.canvas);
        self.num_forks += 1;

        // println!("{:?}", backtrace::Backtrace::new());
    }

    pub fn clear(&mut self, color: Color) {
        self.inner.clear(color);
    }

    // Doesn't take &Polygon, because this is inherently inefficient. If performance matters,
    // upload, cache, and redraw.
    pub fn draw_polygon(&mut self, color: Color, poly: Polygon) {
        GeomBatch::from(vec![(color, poly)]).draw(self);
    }

    pub fn redraw(&mut self, obj: &Drawable) {
        self.inner
            .redraw(obj, &self.uniforms, &self.prerender.inner);
        self.num_draw_calls += 1;

        // println!("{:?}", backtrace::Backtrace::new());
    }

    pub fn redraw_at(&mut self, top_left: ScreenPt, obj: &Drawable) {
        self.fork(Pt2D::new(0.0, 0.0), top_left, 1.0, None);
        self.redraw(obj);
        self.unfork();
    }

    // TODO Stateful API :(
    pub fn enable_clipping(&mut self, rect: ScreenRectangle) {
        let scale_factor = self.prerender.get_scale_factor();
        self.inner.enable_clipping(rect, scale_factor, self.canvas);
    }

    pub fn disable_clipping(&mut self) {
        let scale_factor = self.prerender.get_scale_factor();
        self.inner.disable_clipping(scale_factor, self.canvas);
    }

    // Canvas stuff.

    pub fn draw_mouse_tooltip(&mut self, txt: Text) {
        // Add some padding
        let pad = 5.0;

        let txt_batch = txt.render(self);
        let raw_dims = txt_batch.get_dims();
        let dims = ScreenDims::new(raw_dims.width + 2.0 * pad, raw_dims.height + 2.0 * pad);

        // TODO Maybe also consider the cursor as a valid center
        let pt = dims.top_left_for_corner(
            ScreenPt::new(self.canvas.cursor.x, self.canvas.cursor.y + 20.0),
            &self.canvas,
        );
        let mut batch = GeomBatch::new();
        // TODO Outline?
        batch.push(
            Color::BLACK,
            Polygon::rectangle(dims.width, dims.height).translate(pt.x, pt.y),
        );
        batch.append(txt_batch.translate(pt.x + pad, pt.y + pad));

        // fork_screenspace, but with an even more prominent Z
        self.uniforms.transform = [0.0, 0.0, 1.0];
        self.uniforms.window = [
            self.canvas.window_width as f32,
            self.canvas.window_height as f32,
            TOOLTIP_Z,
        ];
        self.num_forks += 1;
        // Temporarily disable clipping if needed.
        let clip = self
            .inner
            .take_clip(self.prerender.get_scale_factor(), self.canvas);
        batch.draw(self);
        self.unfork();
        self.inner.restore_clip(clip);
    }

    pub fn get_screen_bounds(&self) -> Bounds {
        self.canvas.get_screen_bounds()
    }

    pub fn screen_to_map(&self, pt: ScreenPt) -> Pt2D {
        self.canvas.screen_to_map(pt)
    }

    pub fn get_cursor_in_map_space(&self) -> Option<Pt2D> {
        self.canvas.get_cursor_in_map_space()
    }

    pub(crate) fn get_num_uploads(&self) -> usize {
        self.prerender.num_uploads.get()
    }

    pub fn is_screencap(&self) -> bool {
        self.screencap_mode
    }

    pub fn set_screencap_naming_hint(&mut self, hint: String) {
        assert!(self.screencap_mode);
        assert!(self.naming_hint.is_none());
        self.naming_hint = Some(hint);
    }

    pub fn upload(&mut self, batch: GeomBatch) -> Drawable {
        self.prerender.upload(batch)
    }

    // Delegation to assets
    pub fn default_line_height(&self) -> f64 {
        *self.prerender.assets.default_line_height.borrow()
    }

    pub fn style(&self) -> &Style {
        &self.style
    }

    pub fn is_key_down(&self, key: Key) -> bool {
        self.canvas.keys_held.contains(&key)
    }
}

// TODO Don't expose this directly
// TODO Rename or something maybe. This actually owns all the permanent state of everything.
pub struct Prerender {
    pub(crate) inner: PrerenderInnards,
    pub(crate) assets: Assets,
    pub(crate) num_uploads: Cell<usize>,
    pub(crate) scale_factor: RefCell<f64>,
}

impl Prerender {
    pub fn upload(&self, batch: GeomBatch) -> Drawable {
        self.actually_upload(true, batch)
    }

    pub(crate) fn upload_temporary(&self, batch: GeomBatch) -> Drawable {
        self.actually_upload(false, batch)
    }

    pub fn get_total_bytes_uploaded(&self) -> usize {
        self.inner.total_bytes_uploaded.get()
    }

    fn actually_upload(&self, permanent: bool, batch: GeomBatch) -> Drawable {
        self.num_uploads.set(self.num_uploads.get() + 1);
        self.inner.actually_upload(permanent, batch)

        // println!("{:?}", backtrace::Backtrace::new());
    }

    pub(crate) fn request_redraw(&self) {
        self.inner.request_redraw()
    }

    pub(crate) fn get_scale_factor(&self) -> f64 {
        *self.scale_factor.borrow()
    }

    pub(crate) fn window_size(&self) -> ScreenDims {
        self.inner.window_size(self.get_scale_factor())
    }

    pub(crate) fn window_resized(&self, new_size: ScreenDims) {
        self.inner.window_resized(new_size, self.get_scale_factor())
    }
}

impl std::convert::AsRef<Prerender> for GfxCtx<'_> {
    fn as_ref(&self) -> &Prerender {
        &self.prerender
    }
}

impl std::convert::AsRef<Prerender> for EventCtx<'_> {
    fn as_ref(&self) -> &Prerender {
        &self.prerender
    }
}

impl std::convert::AsRef<Prerender> for Prerender {
    fn as_ref(&self) -> &Prerender {
        self
    }
}
