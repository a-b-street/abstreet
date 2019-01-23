mod canvas;
mod color;
mod event;
mod input;
mod log_scroller;
mod menu;
mod runner;
mod screen_geom;
mod scrolling_menu;
mod text;
mod text_box;
mod top_menu;
mod wizard;

pub use crate::canvas::{Canvas, HorizontalAlignment, VerticalAlignment, BOTTOM_LEFT, CENTERED};
pub use crate::color::Color;
pub use crate::event::{Event, Key};
pub use crate::input::{ModalMenu, UserInput};
pub use crate::log_scroller::LogScroller;
pub use crate::runner::{run, EventLoopMode, GUI};
pub use crate::screen_geom::ScreenPt;
pub use crate::scrolling_menu::ScrollingMenu;
pub use crate::text::Text;
pub use crate::text_box::TextBox;
pub use crate::top_menu::{Folder, TopMenu};
pub use crate::wizard::{Wizard, WrappedWizard};
use geom::Pt2D;
use glium::{implement_vertex, uniform, Surface};

// TODO Not super happy about exposing this; fork_screenspace for external callers should be
// smarter.
pub const TOP_MENU_HEIGHT: f64 = text::LINE_HEIGHT;

pub struct ToggleableLayer {
    layer_name: String,
    // If None, never automatically enable at a certain zoom level.
    min_zoom: Option<f64>,

    enabled: bool,
}

impl ToggleableLayer {
    pub fn new(layer_name: &str, min_zoom: Option<f64>) -> ToggleableLayer {
        ToggleableLayer {
            min_zoom,
            layer_name: layer_name.to_string(),
            enabled: false,
        }
    }

    pub fn is_enabled(&self) -> bool {
        self.enabled
    }

    pub fn handle_zoom(&mut self, before_zoom: f64, after_zoom: f64) {
        if let Some(threshold) = self.min_zoom {
            let before_value = before_zoom >= threshold;
            let after_value = after_zoom >= threshold;
            if before_value != after_value {
                self.enabled = after_value;
            }
        }
    }

    // True if there was a change
    pub fn event(&mut self, input: &mut input::UserInput) -> bool {
        if input.action_chosen(&format!("show/hide {}", self.layer_name)) {
            self.enabled = !self.enabled;
            return true;
        }
        false
    }

    pub fn disable(&mut self) {
        self.enabled = false;
    }
}

pub enum InputResult<T: Clone> {
    Canceled,
    StillActive,
    Done(String, T),
}

#[derive(Copy, Clone)]
struct Vertex {
    position: [f32; 2],
    // TODO Maybe pass color as a uniform instead
    color: [f32; 4],
}

implement_vertex!(Vertex, position, color);

type Uniforms<'a> = glium::uniforms::UniformsStorage<
    'a,
    [f32; 2],
    glium::uniforms::UniformsStorage<'a, [f32; 3], glium::uniforms::EmptyUniforms>,
>;

pub struct GfxCtx<'a> {
    display: &'a glium::Display,
    target: &'a mut glium::Frame,
    program: &'a glium::Program,
    uniforms: Uniforms<'a>,
    params: glium::DrawParameters<'a>,
}

impl<'a> GfxCtx<'a> {
    pub fn new(
        canvas: &Canvas,
        display: &'a glium::Display,
        target: &'a mut glium::Frame,
        program: &'a glium::Program,
    ) -> GfxCtx<'a> {
        let params = glium::DrawParameters {
            depth: glium::Depth {
                test: glium::DepthTest::IfLess,
                write: true,
                ..Default::default()
            },
            ..Default::default()
        };

        let uniforms = uniform! {
            transform: [canvas.cam_x as f32, canvas.cam_y as f32, canvas.cam_zoom as f32],
            window: [canvas.window_width as f32, canvas.window_height as f32],
        };

        GfxCtx {
            display,
            target,
            program,
            uniforms,
            params,
        }
    }

    // Up to the caller to call unfork()!
    // TODO Canvas doesn't understand this change, so things like text drawing that use
    // map_to_screen will just be confusing.
    pub fn fork(&mut self, top_left: Pt2D, zoom: f64) {
        // TODO set uniforms based on values above
    }

    pub fn fork_screenspace(&mut self) {
        self.fork(Pt2D::new(0.0, 0.0), 1.0)
    }

    pub fn unfork(&mut self) {
        // TODO Reset to canvas?
    }

    pub fn clear(&mut self, color: Color) {
        self.target
            .clear_color_and_depth((color.0[0], color.0[1], color.0[2], color.0[3]), 1.0);
    }

    // Use graphics::Line internally for now, but make it easy to switch to something else by
    // picking this API now.
    pub fn draw_line(&mut self, color: Color, thickness: f64, line: &geom::Line) {
        self.draw_polygon(color, &line.to_polyline().make_polygons(thickness));
    }

    pub fn draw_rounded_line(&mut self, color: Color, thickness: f64, line: &geom::Line) {
        self.draw_line(color, thickness, line);
        self.draw_circle(color, &geom::Circle::new(line.pt1(), thickness / 2.0));
        self.draw_circle(color, &geom::Circle::new(line.pt2(), thickness / 2.0));
    }

    pub fn draw_arrow(&mut self, color: Color, thickness: f64, line: &geom::Line) {
        // TODO Raw method doesn't work yet in all cases...
        /*graphics::Line::new_round(color.0, thickness).draw_arrow(
            [
                line.pt1().x(),
                line.pt1().y(),
                line.pt2().x(),
                line.pt2().y(),
            ],
            2.0 * thickness,
            &self.ctx.draw_state,
            self.ctx.transform,
            self.gfx,
        );*/

        /*use dimensioned::si;
        let head_size = 2.0 * thickness;
        let angle = line.angle();
        let triangle_height = (head_size / 2.0).sqrt() * si::M;
        self.draw_polygon(
            color,
            &geom::Polygon::new(&vec![
                //line.pt2(),
                //line.pt2().project_away(head_size, angle.rotate_degs(-135.0)),
                line.reverse()
                    .dist_along(triangle_height)
                    .project_away(thickness / 2.0, angle.rotate_degs(90.0)),
                line.pt1()
                    .project_away(thickness / 2.0, angle.rotate_degs(90.0)),
                line.pt1()
                    .project_away(thickness / 2.0, angle.rotate_degs(-90.0)),
                line.reverse()
                    .dist_along(triangle_height)
                    .project_away(thickness / 2.0, angle.rotate_degs(-90.0)),
                //line.pt2().project_away(head_size, angle.rotate_degs(135.0)),
            ]),
        );
        self.draw_polygon(
            color,
            &geom::Polygon::new(&vec![
                line.pt2(),
                line.pt2()
                    .project_away(head_size, angle.rotate_degs(-135.0)),
                line.pt2().project_away(head_size, angle.rotate_degs(135.0)),
            ]),
        );*/
    }

    pub fn draw_polygon(&mut self, color: Color, poly: &geom::Polygon) {
        for tri in &poly.triangles {
            let vb = glium::VertexBuffer::new(
                self.display,
                &[
                    Vertex {
                        position: [tri.pt1.x() as f32, tri.pt1.y() as f32],
                        color: color.0,
                    },
                    Vertex {
                        position: [tri.pt2.x() as f32, tri.pt2.y() as f32],
                        color: color.0,
                    },
                    Vertex {
                        position: [tri.pt3.x() as f32, tri.pt3.y() as f32],
                        color: color.0,
                    },
                ],
            )
            .unwrap();
            let indices = glium::index::NoIndices(glium::index::PrimitiveType::TrianglesList);

            self.target
                .draw(&vb, &indices, &self.program, &self.uniforms, &self.params)
                .unwrap();
        }
    }

    pub fn draw_circle(&mut self, color: Color, circle: &geom::Circle) {
        /*graphics::Ellipse::new(color.0).draw(
            [
                circle.center.x() - circle.radius,
                circle.center.y() - circle.radius,
                2.0 * circle.radius,
                2.0 * circle.radius,
            ],
            &self.ctx.draw_state,
            self.ctx.transform,
            self.gfx,
        );*/
    }
}
