use crate::{
    text, Canvas, Color, Drawable, HorizontalAlignment, Prerender, ScreenPt, Text,
    VerticalAlignment,
};
use geom::{Bounds, Circle, Distance, Line, Polygon, Pt2D};
use glium::{uniform, Surface};

type Uniforms<'a> = glium::uniforms::UniformsStorage<
    'a,
    [f32; 2],
    glium::uniforms::UniformsStorage<'a, [f32; 3], glium::uniforms::EmptyUniforms>,
>;

pub struct GfxCtx<'a> {
    target: &'a mut glium::Frame,
    program: &'a glium::Program,
    uniforms: Uniforms<'a>,
    params: glium::DrawParameters<'a>,

    // TODO Don't be pub. Delegate everything.
    pub canvas: &'a Canvas,
    pub prerender: &'a Prerender<'a>,

    pub num_draw_calls: usize,
}

impl<'a> GfxCtx<'a> {
    pub(crate) fn new(
        canvas: &'a Canvas,
        prerender: &'a Prerender<'a>,
        target: &'a mut glium::Frame,
        program: &'a glium::Program,
    ) -> GfxCtx<'a> {
        let params = glium::DrawParameters {
            blend: glium::Blend::alpha_blending(),
            ..Default::default()
        };

        let uniforms = uniform! {
            transform: [canvas.cam_x as f32, canvas.cam_y as f32, canvas.cam_zoom as f32],
            window: [canvas.window_width as f32, canvas.window_height as f32],
        };

        GfxCtx {
            canvas,
            prerender,
            target,
            program,
            uniforms,
            params,
            num_draw_calls: 0,
        }
    }

    // Up to the caller to call unfork()!
    // TODO Canvas doesn't understand this change, so things like text drawing that use
    // map_to_screen will just be confusing.
    pub fn fork(&mut self, top_left_map: Pt2D, top_left_screen: ScreenPt, zoom: f64) {
        // map_to_screen of top_left_map should be top_left_screen
        let cam_x = (top_left_map.x() * zoom) - top_left_screen.x;
        let cam_y = (top_left_map.y() * zoom) - top_left_screen.y;

        self.uniforms = uniform! {
            transform: [cam_x as f32, cam_y as f32, zoom as f32],
            window: [self.canvas.window_width as f32, self.canvas.window_height as f32],
        };
    }

    pub fn fork_screenspace(&mut self) {
        self.uniforms = uniform! {
            transform: [0.0, 0.0, 1.0],
            window: [self.canvas.window_width as f32, self.canvas.window_height as f32],
        };
    }

    pub fn unfork(&mut self) {
        self.uniforms = uniform! {
            transform: [self.canvas.cam_x as f32, self.canvas.cam_y as f32, self.canvas.cam_zoom as f32],
            window: [self.canvas.window_width as f32, self.canvas.window_height as f32],
        };
    }

    pub fn clear(&mut self, color: Color) {
        // Without this, SRGB gets enabled and post-processes the color from the fragment shader.
        self.target
            .clear_color_srgb(color.0[0], color.0[1], color.0[2], color.0[3]);
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
        self.draw_polygons(color, &line.to_polyline().make_arrow(thickness).unwrap());
    }

    pub fn draw_circle(&mut self, color: Color, circle: &Circle) {
        self.draw_polygon(color, &circle.to_polygon());
    }

    pub fn draw_polygon(&mut self, color: Color, poly: &Polygon) {
        let obj = self.prerender.upload_temporary(vec![(color, poly)]);
        self.redraw(&obj);
    }

    pub fn draw_polygons(&mut self, color: Color, polygons: &Vec<Polygon>) {
        self.draw_polygon_batch(polygons.iter().map(|p| (color, p)).collect())
    }

    pub fn draw_polygon_batch(&mut self, list: Vec<(Color, &Polygon)>) {
        let obj = self.prerender.upload_temporary(list);
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
    }

    // Canvas stuff.

    // The text box covers up what's beneath and eats the cursor (for get_cursor_in_map_space).
    pub fn draw_blocking_text(
        &mut self,
        txt: &Text,
        (horiz, vert): (HorizontalAlignment, VerticalAlignment),
    ) {
        if txt.is_empty() {
            return;
        }
        let (width, height) = self.text_dims(&txt);
        let x1 = match horiz {
            HorizontalAlignment::Left => 0.0,
            HorizontalAlignment::Center => (self.canvas.window_width - width) / 2.0,
            HorizontalAlignment::Right => self.canvas.window_width - width,
        };
        let y1 = match vert {
            VerticalAlignment::Top => 0.0,
            VerticalAlignment::BelowTopMenu => self.canvas.line_height,
            VerticalAlignment::Center => (self.canvas.window_height - height) / 2.0,
            VerticalAlignment::Bottom => self.canvas.window_height - height,
        };
        self.canvas
            .covered_areas
            .borrow_mut()
            .push(text::draw_text_bubble(
                self,
                ScreenPt::new(x1, y1),
                txt,
                (width, height),
            ));
    }

    pub fn get_screen_bounds(&self) -> Bounds {
        self.canvas.get_screen_bounds()
    }

    // TODO Rename these draw_nonblocking_text_*
    pub fn draw_text_at(&mut self, txt: &Text, map_pt: Pt2D) {
        let (width, height) = self.text_dims(&txt);
        let pt = self.canvas.map_to_screen(map_pt);
        text::draw_text_bubble(
            self,
            ScreenPt::new(pt.x - (width / 2.0), pt.y - (height / 2.0)),
            txt,
            (width, height),
        );
    }

    pub fn text_dims(&self, txt: &Text) -> (f64, f64) {
        self.canvas.text_dims(txt)
    }

    pub fn draw_text_at_screenspace_topleft(&mut self, txt: &Text, pt: ScreenPt) {
        let dims = self.text_dims(&txt);
        text::draw_text_bubble(self, pt, txt, dims);
    }

    pub fn draw_mouse_tooltip(&mut self, txt: &Text) {
        let (width, height) = self.text_dims(&txt);
        let x1 = self.canvas.cursor_x - (width / 2.0);
        let y1 = self.canvas.cursor_y - (height / 2.0);
        // No need to cover the tooltip; this tooltip follows the mouse anyway.
        text::draw_text_bubble(self, ScreenPt::new(x1, y1), txt, (width, height));
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
}
