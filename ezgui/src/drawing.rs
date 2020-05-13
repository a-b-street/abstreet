use crate::assets::Assets;
use crate::backend::{GfxCtxInnards, PrerenderInnards};
use crate::{
    Canvas, Color, Drawable, FancyColor, GeomBatch, ScreenDims, ScreenPt, ScreenRectangle, Style,
    Text,
};
use geom::{ArrowCap, Bounds, Circle, Distance, Line, Polygon, Pt2D};
use std::cell::Cell;

// Lower is more on top
const MAPSPACE_Z: f32 = 1.0;
const SCREENSPACE_Z: f32 = 0.5;
const TOOLTIP_Z: f32 = 0.0;

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

    pub num_draw_calls: usize,
    pub num_forks: usize,
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
    }

    pub fn clear(&mut self, color: Color) {
        self.inner.clear(color);
    }

    pub fn draw_line(&mut self, color: Color, thickness: Distance, line: &Line) {
        self.draw_polygon(color, &line.make_polygons(thickness));
    }

    pub fn draw_rounded_line(&mut self, color: Color, thickness: Distance, line: &Line) {
        self.draw_polygons(
            color,
            &vec![
                line.make_polygons(thickness),
                Circle::new(line.pt1(), thickness / 2.0).to_polygon(),
                Circle::new(line.pt2(), thickness / 2.0).to_polygon(),
            ],
        );
    }

    pub fn draw_arrow(&mut self, color: Color, thickness: Distance, line: &Line) {
        self.draw_polygon(
            color,
            &line
                .to_polyline()
                .make_arrow(thickness, ArrowCap::Triangle)
                .unwrap(),
        );
    }

    pub fn draw_circle(&mut self, color: Color, circle: &Circle) {
        self.draw_polygon(color, &circle.to_polygon());
    }

    pub fn draw_polygon(&mut self, color: Color, poly: &Polygon) {
        let obj = self
            .prerender
            .upload_temporary(vec![(FancyColor::RGBA(color), poly)]);
        self.redraw(&obj);
    }

    pub fn draw_polygons(&mut self, color: Color, polygons: &Vec<Polygon>) {
        let obj = self.prerender.upload_temporary(
            polygons
                .iter()
                .map(|p| (FancyColor::RGBA(color), p))
                .collect(),
        );
        self.redraw(&obj);
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
        self.inner.enable_clipping(rect, self.canvas);
    }

    pub fn disable_clipping(&mut self) {
        self.inner.disable_clipping(self.canvas);
    }

    // Canvas stuff.

    pub fn draw_mouse_tooltip(&mut self, txt: Text) {
        // Add some padding
        let pad = 5.0;

        let txt_batch = txt.render_g(self);
        let raw_dims = txt_batch.get_dims();
        let dims = ScreenDims::new(raw_dims.width + 2.0 * pad, raw_dims.height + 2.0 * pad);

        // TODO Maybe also consider the cursor as a valid center
        let pt = dims.top_left_for_corner(
            ScreenPt::new(self.canvas.cursor_x, self.canvas.cursor_y + 20.0),
            &self.canvas,
        );
        let mut batch = GeomBatch::new();
        // TODO Outline?
        batch.push(
            Color::BLACK,
            Polygon::rectangle(dims.width, dims.height).translate(pt.x, pt.y),
        );
        batch.add_translated(txt_batch, pt.x + pad, pt.y + pad);

        // fork_screenspace, but with an even more prominent Z
        self.uniforms.transform = [0.0, 0.0, 1.0];
        self.uniforms.window = [
            self.canvas.window_width as f32,
            self.canvas.window_height as f32,
            TOOLTIP_Z,
        ];
        self.num_forks += 1;
        // Temporarily disable clipping if needed.
        let clip = self.inner.take_clip();
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

    pub fn get_num_uploads(&self) -> usize {
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
}

// TODO Don't expose this directly
// TODO Rename or something maybe. This actually owns all the permanent state of everything.
pub struct Prerender {
    pub(crate) inner: PrerenderInnards,
    pub(crate) assets: Assets,
    pub(crate) num_uploads: Cell<usize>,
}

impl Prerender {
    pub fn upload(&self, batch: GeomBatch) -> Drawable {
        let borrows = batch.list.iter().map(|(c, p)| (c.clone(), p)).collect();
        self.actually_upload(true, borrows)
    }

    pub fn get_total_bytes_uploaded(&self) -> usize {
        self.inner.total_bytes_uploaded.get()
    }

    pub(crate) fn upload_temporary(&self, list: Vec<(FancyColor, &Polygon)>) -> Drawable {
        self.actually_upload(false, list)
    }

    fn actually_upload(&self, permanent: bool, list: Vec<(FancyColor, &Polygon)>) -> Drawable {
        // println!("{:?}", backtrace::Backtrace::new());
        self.num_uploads.set(self.num_uploads.get() + 1);
        self.inner.actually_upload(permanent, list)
    }

    pub fn request_redraw(&self) {
        self.inner.request_redraw()
    }
}
