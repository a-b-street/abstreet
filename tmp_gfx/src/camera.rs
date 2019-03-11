use glium::glutin;
use std::f32;

pub struct CameraState {
    aspect_ratio: f32,
    position: (f32, f32, f32),
    direction: (f32, f32, f32),

    moving_up: bool,
    moving_left: bool,
    moving_down: bool,
    moving_right: bool,
    moving_forward: bool,
    moving_backward: bool,
}

impl CameraState {
    pub fn new() -> CameraState {
        CameraState {
            aspect_ratio: 1024.0 / 768.0,
            position: (0.1, 0.1, 1.0),
            direction: (0.0, 0.0, -1.0),
            moving_up: false,
            moving_left: false,
            moving_down: false,
            moving_right: false,
            moving_forward: false,
            moving_backward: false,
        }
    }

    pub fn get_perspective(&self) -> [[f32; 4]; 4] {
        let fov = f32::consts::PI / 2.0;
        let zfar = 1024.0;
        let znear = 0.1;

        let f = 1.0 / (fov / 2.0).tan();

        // note: remember that this is column-major, so the lines of code are actually columns
        [
            [f / self.aspect_ratio, 0.0, 0.0, 0.0],
            [0.0, f, 0.0, 0.0],
            [0.0, 0.0, (zfar + znear) / (zfar - znear), 1.0],
            [0.0, 0.0, -(2.0 * zfar * znear) / (zfar - znear), 0.0],
        ]
    }

    pub fn get_view(&self) -> [[f32; 4]; 4] {
        let f = {
            let f = self.direction;
            let len = f.0 * f.0 + f.1 * f.1 + f.2 * f.2;
            let len = len.sqrt();
            (f.0 / len, f.1 / len, f.2 / len)
        };

        let up = (0.0, 1.0, 0.0);

        let s = (
            f.1 * up.2 - f.2 * up.1,
            f.2 * up.0 - f.0 * up.2,
            f.0 * up.1 - f.1 * up.0,
        );

        let s_norm = {
            let len = s.0 * s.0 + s.1 * s.1 + s.2 * s.2;
            let len = len.sqrt();
            (s.0 / len, s.1 / len, s.2 / len)
        };

        let u = (
            s_norm.1 * f.2 - s_norm.2 * f.1,
            s_norm.2 * f.0 - s_norm.0 * f.2,
            s_norm.0 * f.1 - s_norm.1 * f.0,
        );

        let p = (
            -self.position.0 * s.0 - self.position.1 * s.1 - self.position.2 * s.2,
            -self.position.0 * u.0 - self.position.1 * u.1 - self.position.2 * u.2,
            -self.position.0 * f.0 - self.position.1 * f.1 - self.position.2 * f.2,
        );

        // note: remember that this is column-major, so the lines of code are actually columns
        [
            [s_norm.0, u.0, f.0, 0.0],
            [s_norm.1, u.1, f.1, 0.0],
            [s_norm.2, u.2, f.2, 0.0],
            [p.0, p.1, p.2, 1.0],
        ]
    }

    fn update(&mut self) {
        let f = {
            let f = self.direction;
            let len = f.0 * f.0 + f.1 * f.1 + f.2 * f.2;
            let len = len.sqrt();
            (f.0 / len, f.1 / len, f.2 / len)
        };

        let up = (0.0, 1.0, 0.0);

        let s = (
            f.1 * up.2 - f.2 * up.1,
            f.2 * up.0 - f.0 * up.2,
            f.0 * up.1 - f.1 * up.0,
        );

        let s = {
            let len = s.0 * s.0 + s.1 * s.1 + s.2 * s.2;
            let len = len.sqrt();
            (s.0 / len, s.1 / len, s.2 / len)
        };

        let u = (
            s.1 * f.2 - s.2 * f.1,
            s.2 * f.0 - s.0 * f.2,
            s.0 * f.1 - s.1 * f.0,
        );

        let speed = 0.1;

        if self.moving_up {
            self.position.0 += u.0 * speed;
            self.position.1 += u.1 * speed;
            self.position.2 += u.2 * speed;
        }

        if self.moving_left {
            self.position.0 -= s.0 * speed;
            self.position.1 -= s.1 * speed;
            self.position.2 -= s.2 * speed;
        }

        if self.moving_down {
            self.position.0 -= u.0 * speed;
            self.position.1 -= u.1 * speed;
            self.position.2 -= u.2 * speed;
        }

        if self.moving_right {
            self.position.0 += s.0 * speed;
            self.position.1 += s.1 * speed;
            self.position.2 += s.2 * speed;
        }

        if self.moving_forward {
            self.position.0 += f.0 * speed;
            self.position.1 += f.1 * speed;
            self.position.2 += f.2 * speed;
        }

        if self.moving_backward {
            self.position.0 -= f.0 * speed;
            self.position.1 -= f.1 * speed;
            self.position.2 -= f.2 * speed;
        }
    }

    pub fn process_input(&mut self, input: glutin::KeyboardInput) {
        let pressed = input.state == glutin::ElementState::Pressed;
        match input.virtual_keycode {
            Some(glutin::VirtualKeyCode::Up) => self.moving_up = pressed,
            Some(glutin::VirtualKeyCode::Down) => self.moving_down = pressed,
            Some(glutin::VirtualKeyCode::Left) => self.moving_left = pressed,
            Some(glutin::VirtualKeyCode::Right) => self.moving_right = pressed,
            Some(glutin::VirtualKeyCode::Q) => self.moving_forward = pressed,
            Some(glutin::VirtualKeyCode::A) => self.moving_backward = pressed,
            _ => {}
        };
        self.update();
    }
}
