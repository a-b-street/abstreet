use crate::assets::Assets;
use crate::svg;
use crate::{
    text, Canvas, Color, EventCtx, HorizontalAlignment, ScreenDims, ScreenPt, ScreenRectangle,
    Text, VerticalAlignment,
};
use geom::{Angle, Bounds, Circle, Distance, Line, Polygon, Pt2D};
use glium::uniforms::{SamplerBehavior, SamplerWrapFunction, UniformValue};
use glium::Surface;
use glium_glyph::glyph_brush::FontId;
use std::cell::Cell;

const MAPSPACE: f32 = 0.0;
const SCREENSPACE: f32 = 1.0;

struct Uniforms<'a> {
    // (cam_x, cam_y, cam_zoom)
    transform: [f32; 3],
    // (window_width, window_height, 0.0 for mapspace or 1.0 for screenspace)
    window: [f32; 3],
    canvas: &'a Canvas,
}

impl<'a> Uniforms<'a> {
    fn new(canvas: &'a Canvas) -> Uniforms<'a> {
        Uniforms {
            transform: [
                canvas.cam_x as f32,
                canvas.cam_y as f32,
                canvas.cam_zoom as f32,
            ],
            window: [
                canvas.window_width as f32,
                canvas.window_height as f32,
                MAPSPACE,
            ],
            canvas,
        }
    }
}

impl<'b> glium::uniforms::Uniforms for Uniforms<'b> {
    fn visit_values<'a, F: FnMut(&str, UniformValue<'a>)>(&'a self, mut output: F) {
        output("transform", UniformValue::Vec3(self.transform));
        output("window", UniformValue::Vec3(self.window));

        // This is fine to use for all of the texture styles; all but non-tiling textures clamp to
        // [0, 1] anyway.
        let tile = SamplerBehavior {
            wrap_function: (
                SamplerWrapFunction::Repeat,
                SamplerWrapFunction::Repeat,
                SamplerWrapFunction::Repeat,
            ),
            ..Default::default()
        };
        for (idx, tex) in self.canvas.texture_arrays.iter().enumerate() {
            output(
                &format!("tex{}", idx),
                UniformValue::Texture2dArray(tex, Some(tile)),
            );
        }
    }
}

pub struct GfxCtx<'a> {
    pub(crate) target: &'a mut glium::Frame,
    program: &'a glium::Program,
    uniforms: Uniforms<'a>,
    params: glium::DrawParameters<'a>,

    screencap_mode: bool,
    pub(crate) naming_hint: Option<String>,

    // TODO Don't be pub. Delegate everything.
    pub canvas: &'a Canvas,
    pub prerender: &'a Prerender<'a>,
    pub(crate) assets: &'a Assets,

    pub num_draw_calls: usize,
}

impl<'a> GfxCtx<'a> {
    pub(crate) fn new(
        canvas: &'a Canvas,
        prerender: &'a Prerender<'a>,
        target: &'a mut glium::Frame,
        program: &'a glium::Program,
        assets: &'a Assets,
        screencap_mode: bool,
    ) -> GfxCtx<'a> {
        let params = glium::DrawParameters {
            blend: glium::Blend::alpha_blending(),
            depth: glium::Depth {
                test: glium::DepthTest::IfLessOrEqual,
                write: true,
                ..Default::default()
            },
            ..Default::default()
        };

        let uniforms = Uniforms::new(&canvas);

        GfxCtx {
            canvas,
            prerender,
            target,
            program,
            uniforms,
            params,
            num_draw_calls: 0,
            screencap_mode,
            naming_hint: None,
            assets,
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
            SCREENSPACE,
        ];
    }

    pub fn fork_screenspace(&mut self) {
        self.uniforms.transform = [0.0, 0.0, 1.0];
        self.uniforms.window = [
            self.canvas.window_width as f32,
            self.canvas.window_height as f32,
            SCREENSPACE,
        ];
    }

    pub fn unfork(&mut self) {
        self.uniforms = Uniforms::new(&self.canvas);
    }

    pub fn clear(&mut self, color: Color) {
        match color {
            Color::RGBA(r, g, b, a) => {
                // Without this, SRGB gets enabled and post-processes the color from the fragment
                // shader.
                self.target.clear_color_srgb_and_depth((r, g, b, a), 1.0);
            }
            _ => unreachable!(),
        }
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
        self.target
            .draw(
                &obj.vertex_buffer,
                &obj.index_buffer,
                &self.program,
                &self.uniforms,
                &self.params,
            )
            .unwrap();
        self.num_draw_calls += 1;

        // println!("{:?}", backtrace::Backtrace::new());
    }

    // TODO Stateful API :(
    pub fn enable_clipping(&mut self, rect: ScreenRectangle) {
        assert!(self.params.scissor.is_none());
        self.params.scissor = Some(glium::Rect {
            left: (self.canvas.hidpi_factor * rect.x1) as u32,
            // Y-inversion
            bottom: (self.canvas.hidpi_factor * (self.canvas.window_height - rect.y2)) as u32,
            width: (self.canvas.hidpi_factor * (rect.x2 - rect.x1)) as u32,
            height: (self.canvas.hidpi_factor * (rect.y2 - rect.y1)) as u32,
        });
    }

    pub fn disable_clipping(&mut self) {
        assert!(self.params.scissor.is_some());
        self.params.scissor = None;
    }

    // Canvas stuff.

    // The text box covers up what's beneath and eats the cursor (for get_cursor_in_map_space).
    pub fn draw_blocking_text(
        &mut self,
        txt: &Text,
        (horiz, vert): (HorizontalAlignment, VerticalAlignment),
    ) {
        let mut dims = self.text_dims(&txt);
        let top_left = self.canvas.align_window(dims, horiz, vert);
        if let HorizontalAlignment::FillScreen = horiz {
            dims.width = self.canvas.window_width;
        }
        self.canvas
            .mark_covered_area(text::draw_text_bubble(self, top_left, txt, dims));
    }

    pub fn get_screen_bounds(&self) -> Bounds {
        self.canvas.get_screen_bounds()
    }

    // TODO Rename these draw_nonblocking_text_*
    pub fn draw_text_at(&mut self, txt: &Text, map_pt: Pt2D) {
        let dims = self.text_dims(&txt);
        let pt = self.canvas.map_to_screen(map_pt);
        text::draw_text_bubble(
            self,
            ScreenPt::new(pt.x - (dims.width / 2.0), pt.y - (dims.height / 2.0)),
            txt,
            dims,
        );
    }

    pub fn draw_text_at_mapspace(&mut self, txt: &Text, map_pt: Pt2D) {
        let dims = self.text_dims(&txt);
        text::draw_text_bubble_mapspace(
            self,
            Pt2D::new(
                map_pt.x() - (dims.width / (2.0 * text::SCALE_DOWN)),
                map_pt.y() - (dims.height / (2.0 * text::SCALE_DOWN)),
            ),
            txt,
            dims,
        );
    }

    pub fn draw_text_at_screenspace_topleft(&mut self, txt: &Text, pt: ScreenPt) {
        let dims = self.text_dims(&txt);
        self.canvas
            .mark_covered_area(text::draw_text_bubble(self, pt, txt, dims));
    }

    pub fn draw_mouse_tooltip(&mut self, txt: &Text) {
        let dims = self.text_dims(&txt);
        // TODO Maybe also consider the cursor as a valid center
        let pt = dims.top_left_for_corner(
            ScreenPt::new(self.canvas.cursor_x, self.canvas.cursor_y),
            &self.canvas,
        );
        // No need to cover the tooltip; this tooltip follows the mouse anyway.
        text::draw_text_bubble(self, pt, txt, dims);
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

    pub fn button_tooltip(&self) -> Option<Text> {
        self.canvas.button_tooltip.clone()
    }

    // Delegation to assets
    pub fn default_line_height(&self) -> f64 {
        self.assets.default_line_height
    }
    pub(crate) fn line_height(&self, font: FontId, font_size: usize) -> f64 {
        self.assets.line_height(font, font_size)
    }
    pub fn text_dims(&self, txt: &Text) -> ScreenDims {
        self.assets.text_dims(txt)
    }
}

#[derive(Clone)]
pub struct GeomBatch {
    list: Vec<(Color, Polygon)>,
}

impl GeomBatch {
    pub fn new() -> GeomBatch {
        GeomBatch { list: Vec::new() }
    }

    pub fn from(list: Vec<(Color, Polygon)>) -> GeomBatch {
        GeomBatch { list }
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

    pub(crate) fn get_dims(&self) -> ScreenDims {
        let mut bounds = Bounds::new();
        for (_, poly) in &self.list {
            bounds.union(poly.get_bounds());
        }
        ScreenDims::new(bounds.width(), bounds.height())
    }

    // Slightly weird use case, but hotswap colors.
    pub fn rewrite_color(&mut self, transformation: RewriteColor) {
        for (c, _) in self.list.iter_mut() {
            match transformation {
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
    pub fn add_svg(&mut self, filename: &str, center: Pt2D, scale: f64, rotate: Angle) {
        let mut batch = GeomBatch::new();
        svg::add_svg(&mut batch, filename);
        let dims = batch.get_dims();
        let dx = center.x() - dims.width * scale / 2.0;
        let dy = center.y() - dims.height * scale / 2.0;
        for (color, poly) in batch.consume() {
            self.push(color, poly.scale(scale).translate(dx, dy).rotate(rotate));
        }
    }
}

pub enum RewriteColor {
    Change(Color, Color),
    ChangeAll(Color),
}

// Something that's been sent to the GPU already.
pub struct Drawable {
    vertex_buffer: glium::VertexBuffer<Vertex>,
    index_buffer: glium::IndexBuffer<u32>,
}

#[derive(Copy, Clone)]
pub(crate) struct Vertex {
    position: [f32; 2],
    // Each type of Color encodes something different here. See the actually_upload method and
    // fragment_140.glsl.
    // TODO Make this u8?
    style: [f32; 4],
}

glium::implement_vertex!(Vertex, position, style);

// TODO Don't expose this directly
pub struct Prerender<'a> {
    pub(crate) display: &'a glium::Display,
    pub(crate) num_uploads: Cell<usize>,
    // TODO Prerender doesn't know what things are temporary and permanent. Could make the API more
    // detailed (and use the corresponding persistent glium types).
    pub(crate) total_bytes_uploaded: Cell<usize>,
}

impl<'a> Prerender<'a> {
    pub fn upload_borrowed(&self, list: Vec<(Color, &Polygon)>) -> Drawable {
        self.actually_upload(true, list)
    }

    pub fn upload(&self, batch: GeomBatch) -> Drawable {
        let borrows = batch.list.iter().map(|(c, p)| (*c, p)).collect();
        self.actually_upload(true, borrows)
    }

    pub fn get_total_bytes_uploaded(&self) -> usize {
        self.total_bytes_uploaded.get()
    }

    pub(crate) fn upload_temporary(&self, list: Vec<(Color, &Polygon)>) -> Drawable {
        self.actually_upload(false, list)
    }

    fn actually_upload(&self, permanent: bool, list: Vec<(Color, &Polygon)>) -> Drawable {
        // println!("{:?}", backtrace::Backtrace::new());

        self.num_uploads.set(self.num_uploads.get() + 1);

        let mut vertices: Vec<Vertex> = Vec::new();
        let mut indices: Vec<u32> = Vec::new();

        for (color, poly) in list {
            let idx_offset = vertices.len();
            let (pts, raw_indices, maybe_uv) = poly.raw_for_rendering();
            for (idx, pt) in pts.iter().enumerate() {
                // For the three texture cases, pass [U coordinate, V coordinate, texture group ID,
                // 100 + texture offset ID] as the style. The last field is between 0 an 1 RGBA's
                // alpha values, so bump by 100 to distinguish from that.
                let style = match color {
                    Color::RGBA(r, g, b, a) => [r, g, b, a],
                    Color::TileTexture(id, tex_dims) => {
                        // The texture uses SamplerWrapFunction::Repeat, so don't clamp to [0, 1].
                        // Also don't offset based on the polygon's bounds -- even if there are
                        // separate but adjacent polygons, we want seamless tiling.
                        let tx = pt.x() / tex_dims.width;
                        let ty = pt.y() / tex_dims.height;
                        [tx as f32, ty as f32, id.0, 100.0 + id.1]
                    }
                    Color::StretchTexture(id, _, angle) => {
                        // TODO Cache
                        let b = poly.get_bounds();
                        let center = poly.center();
                        let origin_pt = Pt2D::new(pt.x() - center.x(), pt.y() - center.y());
                        let (sin, cos) = angle.invert_y().normalized_radians().sin_cos();
                        let rot_pt = Pt2D::new(
                            center.x() + origin_pt.x() * cos - origin_pt.y() * sin,
                            center.y() + origin_pt.y() * cos + origin_pt.x() * sin,
                        );

                        let tx = (rot_pt.x() - b.min_x) / b.width();
                        let ty = (rot_pt.y() - b.min_y) / b.height();
                        [tx as f32, ty as f32, id.0, 100.0 + id.1]
                    }
                    Color::CustomUVTexture(id) => {
                        let (tx, ty) =
                            maybe_uv.expect("CustomUVTexture with polygon lacking UV")[idx];
                        [tx, ty, id.0, 100.0 + id.1]
                    }
                    // Two final special cases
                    Color::HatchingStyle1 => [100.0, 0.0, 0.0, 0.0],
                    Color::HatchingStyle2 => [101.0, 0.0, 0.0, 0.0],
                };
                vertices.push(Vertex {
                    position: [pt.x() as f32, pt.y() as f32],
                    style,
                });
            }
            for idx in raw_indices {
                indices.push((idx_offset + *idx) as u32);
            }
        }

        let vertex_buffer = if permanent {
            glium::VertexBuffer::immutable(self.display, &vertices).unwrap()
        } else {
            glium::VertexBuffer::new(self.display, &vertices).unwrap()
        };
        let index_buffer = if permanent {
            glium::IndexBuffer::immutable(
                self.display,
                glium::index::PrimitiveType::TrianglesList,
                &indices,
            )
            .unwrap()
        } else {
            glium::IndexBuffer::new(
                self.display,
                glium::index::PrimitiveType::TrianglesList,
                &indices,
            )
            .unwrap()
        };

        if permanent {
            self.total_bytes_uploaded.set(
                self.total_bytes_uploaded.get()
                    + vertex_buffer.get_size()
                    + index_buffer.get_size(),
            );
        }

        Drawable {
            vertex_buffer,
            index_buffer,
        }
    }
}

// I'm tempted to fold this into GeomBatch and Drawable, but since this represents a screen-space
// thing, it'd be weird to do that.
pub struct DrawBoth {
    geom: Drawable,
    txt: Vec<(Text, ScreenPt)>,
    // Covers both geometry and text
    dims: ScreenDims,
}

impl DrawBoth {
    pub fn new(ctx: &EventCtx, batch: GeomBatch, txt: Vec<(Text, ScreenPt)>) -> DrawBoth {
        let mut total_dims = batch.get_dims();
        for (t, pt) in &txt {
            let dims = ctx.text_dims(t);
            let w = dims.width + pt.x;
            let h = dims.height + pt.y;
            if w > total_dims.width {
                total_dims.width = w;
            }
            if h > total_dims.height {
                total_dims.height = h;
            }
        }
        DrawBoth {
            geom: batch.upload(ctx),
            txt,
            dims: total_dims,
        }
    }

    // DON'T fork before calling this.
    pub fn redraw(&self, top_left: ScreenPt, g: &mut GfxCtx) {
        g.fork(Pt2D::new(0.0, 0.0), top_left, 1.0);
        g.redraw(&self.geom);
        g.unfork();
        for (txt, pt) in &self.txt {
            g.draw_text_at_screenspace_topleft(
                txt,
                ScreenPt::new(top_left.x + pt.x, top_left.y + pt.y),
            );
        }
    }

    pub fn get_dims(&self) -> ScreenDims {
        self.dims
    }

    // TODO Hack
    pub(crate) fn override_bounds(&mut self, b: Bounds) {
        self.dims = ScreenDims::new(b.width(), b.height());
    }
}
