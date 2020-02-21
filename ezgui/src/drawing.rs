use crate::assets::Assets;
use crate::backend::{GfxCtxInnards, PrerenderInnards};
use crate::svg;
use crate::{
    Canvas, Color, Drawable, EventCtx, HorizontalAlignment, ScreenDims, ScreenPt, ScreenRectangle,
    Text, VerticalAlignment,
};
use geom::{Angle, Bounds, Circle, Distance, Line, Polygon, Pt2D};
use std::cell::Cell;

// Lower is more on top
const MAPSPACE_Z: f32 = 1.0;
const SCREENSPACE_Z: f32 = 0.5;
const TOOLTIP_Z: f32 = 0.0;

pub struct Uniforms {
    // (cam_x, cam_y, cam_zoom)
    pub transform: [f32; 3],
    // (window_width, window_height, 0.0 for mapspace or 1.0 for screenspace)
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

    pub num_draw_calls: usize,
    pub num_forks: usize,
}

impl<'a> GfxCtx<'a> {
    pub(crate) fn new(
        prerender: &'a Prerender,
        canvas: &'a Canvas,
        screencap_mode: bool,
    ) -> GfxCtx<'a> {
        let uniforms = Uniforms::new(canvas);
        GfxCtx {
            inner: prerender.inner.draw_new_frame(),
            uniforms,
            canvas,
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
    pub fn fork(&mut self, top_left_map: Pt2D, top_left_screen: ScreenPt, zoom: f64) {
        // map_to_screen of top_left_map should be top_left_screen
        let cam_x = (top_left_map.x() * zoom) - top_left_screen.x;
        let cam_y = (top_left_map.y() * zoom) - top_left_screen.y;

        self.uniforms.transform = [cam_x as f32, cam_y as f32, zoom as f32];
        self.uniforms.window = [
            self.canvas.window_width as f32,
            self.canvas.window_height as f32,
            SCREENSPACE_Z,
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
        self.draw_polygon(color, &line.to_polyline().make_arrow(thickness).unwrap());
    }

    pub fn draw_circle(&mut self, color: Color, circle: &Circle) {
        self.draw_polygon(color, &circle.to_polygon());
    }

    pub fn draw_polygon(&mut self, color: Color, poly: &Polygon) {
        let obj = self.prerender.upload_temporary(vec![(color, poly)]);
        self.redraw(&obj);
    }

    pub fn draw_polygons(&mut self, color: Color, polygons: &Vec<Polygon>) {
        let obj = self
            .prerender
            .upload_temporary(polygons.iter().map(|p| (color, p)).collect());
        self.redraw(&obj);
    }

    pub fn redraw(&mut self, obj: &Drawable) {
        self.inner
            .redraw(obj, &self.uniforms, &self.prerender.inner);
        self.num_draw_calls += 1;

        // println!("{:?}", backtrace::Backtrace::new());
    }

    pub fn redraw_at(&mut self, top_left: ScreenPt, obj: &Drawable) {
        self.fork(Pt2D::new(0.0, 0.0), top_left, 1.0);
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

    // The text box covers up what's beneath and eats the cursor (for get_cursor_in_map_space).
    pub fn draw_blocking_text(
        &mut self,
        txt: Text,
        (horiz, vert): (HorizontalAlignment, VerticalAlignment),
    ) {
        let batch = txt.render_g(self);
        let dims = batch.get_dims();
        let top_left = self.canvas.align_window(dims, horiz, vert);

        self.canvas
            .mark_covered_area(ScreenRectangle::top_left(top_left, dims));
        let draw = self.upload(batch);
        self.redraw_at(top_left, &draw);
    }

    pub(crate) fn draw_blocking_text_at_screenspace_topleft(&mut self, txt: Text, pt: ScreenPt) {
        let batch = txt.render_g(self);
        self.canvas
            .mark_covered_area(ScreenRectangle::top_left(pt, batch.get_dims()));
        let draw = self.upload(batch);
        self.redraw_at(pt, &draw);
    }

    // TODO Rename these draw_nonblocking_text_*
    pub fn draw_text_at(&mut self, txt: Text, map_pt: Pt2D) {
        let batch = txt.render_g(self);
        let dims = batch.get_dims();
        let pt = self.canvas.map_to_screen(map_pt);
        let draw = self.upload(batch);
        self.redraw_at(
            ScreenPt::new(pt.x - (dims.width / 2.0), pt.y - (dims.height / 2.0)),
            &draw,
        );
    }

    pub fn draw_mouse_tooltip(&mut self, txt: Text) {
        let txt_batch = txt.render_g(self);
        let dims = txt_batch.get_dims();
        // TODO Maybe also consider the cursor as a valid center
        let pt = dims.top_left_for_corner(
            ScreenPt::new(self.canvas.cursor_x, self.canvas.cursor_y + 20.0),
            &self.canvas,
        );
        let mut batch = GeomBatch::new();
        batch.add_translated(txt_batch, pt.x, pt.y);

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
        self.prerender.assets.default_line_height
    }
}

#[derive(Clone)]
pub struct GeomBatch {
    list: Vec<(Color, Polygon)>,
    // TODO A weird hack for text.
    pub(crate) dims_text: bool,
}

impl GeomBatch {
    pub fn new() -> GeomBatch {
        GeomBatch {
            list: Vec::new(),
            dims_text: false,
        }
    }

    pub fn from(list: Vec<(Color, Polygon)>) -> GeomBatch {
        GeomBatch {
            list,
            dims_text: false,
        }
    }

    pub fn push(&mut self, color: Color, p: Polygon) {
        self.list.push((color, p));
    }

    pub fn extend(&mut self, color: Color, polys: Vec<Polygon>) {
        for p in polys {
            self.list.push((color, p));
        }
    }

    pub fn append(&mut self, other: GeomBatch) {
        self.list.extend(other.list);
    }

    pub fn consume(self) -> Vec<(Color, Polygon)> {
        self.list
    }

    pub fn draw(self, g: &mut GfxCtx) {
        let refs = self.list.iter().map(|(color, p)| (*color, p)).collect();
        let obj = g.prerender.upload_temporary(refs);
        g.redraw(&obj);
    }

    pub fn upload(self, ctx: &EventCtx) -> Drawable {
        ctx.prerender.upload(self)
    }

    // Sets the top-left to 0, 0. Not sure exactly when this should be used.
    pub(crate) fn autocrop(mut self) -> GeomBatch {
        let mut bounds = Bounds::new();
        for (_, poly) in &self.list {
            bounds.union(poly.get_bounds());
        }
        if bounds.min_x == 0.0 && bounds.min_y == 0.0 {
            return self;
        }
        for (_, poly) in &mut self.list {
            *poly = poly.translate(-bounds.min_x, -bounds.min_y);
        }
        self
    }

    pub(crate) fn is_empty(&self) -> bool {
        self.list.is_empty()
    }

    pub fn get_dims(&self) -> ScreenDims {
        // TODO Maybe warn about this happening and avoid in the first place? Sometimes we wind up
        // trying to draw completely empty text.
        if self.is_empty() {
            return ScreenDims::new(0.0, 0.0);
        }
        let mut bounds = Bounds::new();
        for (_, poly) in &self.list {
            bounds.union(poly.get_bounds());
        }
        if self.dims_text {
            ScreenDims::new(bounds.max_x, bounds.max_y)
        } else {
            ScreenDims::new(bounds.width(), bounds.height())
        }
    }

    // Slightly weird use case, but hotswap colors.
    pub fn rewrite_color(&mut self, transformation: RewriteColor) {
        for (c, _) in self.list.iter_mut() {
            match transformation {
                RewriteColor::NoOp => {}
                RewriteColor::Change(from, to) => {
                    if *c == from {
                        *c = to;
                    }
                }
                RewriteColor::ChangeAll(to) => {
                    *c = to;
                }
            }
        }
    }

    // TODO Weird API...
    pub fn add_svg(
        &mut self,
        prerender: &Prerender,
        filename: &str,
        center: Pt2D,
        scale: f64,
        rotate: Angle,
    ) {
        self.add_transformed(svg::load_svg(prerender, filename).0, center, scale, rotate);
    }

    // This centers on the pt!
    pub fn add_transformed(&mut self, other: GeomBatch, center: Pt2D, scale: f64, rotate: Angle) {
        let dims = other.get_dims();
        let dx = center.x() - dims.width * scale / 2.0;
        let dy = center.y() - dims.height * scale / 2.0;
        for (color, mut poly) in other.consume() {
            // Avoid unnecessary transformations for slight perf boost
            if scale != 1.0 {
                poly = poly.scale(scale);
            }
            poly = poly.translate(dx, dy);
            if rotate != Angle::ZERO {
                poly = poly.rotate(rotate);
            }
            self.push(color, poly);
        }
    }

    pub fn add_translated(&mut self, other: GeomBatch, dx: f64, dy: f64) {
        for (color, poly) in other.consume() {
            self.push(color, poly.translate(dx, dy));
        }
    }
}

pub enum RewriteColor {
    NoOp,
    Change(Color, Color),
    ChangeAll(Color),
}

// TODO Don't expose this directly
// TODO Rename or something maybe. This actually owns all the permanent state of everything.
pub struct Prerender {
    pub(crate) inner: PrerenderInnards,
    pub(crate) assets: Assets,
    pub(crate) num_uploads: Cell<usize>,
}

impl Prerender {
    pub fn upload_borrowed(&self, list: Vec<(Color, &Polygon)>) -> Drawable {
        self.actually_upload(true, list)
    }

    pub fn upload(&self, batch: GeomBatch) -> Drawable {
        let borrows = batch.list.iter().map(|(c, p)| (*c, p)).collect();
        self.actually_upload(true, borrows)
    }

    pub fn get_total_bytes_uploaded(&self) -> usize {
        self.inner.total_bytes_uploaded.get()
    }

    pub(crate) fn upload_temporary(&self, list: Vec<(Color, &Polygon)>) -> Drawable {
        self.actually_upload(false, list)
    }

    fn actually_upload(&self, permanent: bool, list: Vec<(Color, &Polygon)>) -> Drawable {
        // println!("{:?}", backtrace::Backtrace::new());
        self.num_uploads.set(self.num_uploads.get() + 1);
        self.inner.actually_upload(permanent, list)
    }

    pub fn request_redraw(&self) {
        self.inner.request_redraw()
    }
}
