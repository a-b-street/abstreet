use crate::ScreenPt;
use geom::Duration;
use winit::event::{
    ElementState, KeyboardInput, MouseButton, MouseScrollDelta, VirtualKeyCode, WindowEvent,
};

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Event {
    // Used to initialize the application and also to recalculate menu state when some other event
    // is used.
    NoOp,
    LeftMouseButtonDown,
    LeftMouseButtonUp,
    RightMouseButtonDown,
    RightMouseButtonUp,
    // TODO KeyDown and KeyUp might be nicer, but piston (and probably X.org) hands over repeated
    // events while a key is held down.
    KeyPress(Key),
    KeyRelease(Key),
    // Some real amount of time has passed since the last update; EventLoopMode::Animation is
    // active
    Update(Duration),
    MouseMovedTo(ScreenPt),
    WindowLostCursor,
    WindowGainedCursor,
    MouseWheelScroll(f64, f64),
    WindowResized(f64, f64),
}

impl Event {
    pub fn from_winit_event(ev: WindowEvent) -> Option<Event> {
        match ev {
            WindowEvent::MouseInput { state, button, .. } => match (button, state) {
                (MouseButton::Left, ElementState::Pressed) => Some(Event::LeftMouseButtonDown),
                (MouseButton::Left, ElementState::Released) => Some(Event::LeftMouseButtonUp),
                (MouseButton::Right, ElementState::Pressed) => Some(Event::RightMouseButtonDown),
                (MouseButton::Right, ElementState::Released) => Some(Event::RightMouseButtonUp),
                _ => None,
            },
            WindowEvent::KeyboardInput { input, .. } => {
                if let Some(key) = Key::from_winit_key(input) {
                    if input.state == ElementState::Pressed {
                        Some(Event::KeyPress(key))
                    } else {
                        Some(Event::KeyRelease(key))
                    }
                } else {
                    None
                }
            }
            WindowEvent::CursorMoved { position, .. } => {
                Some(Event::MouseMovedTo(ScreenPt::new(position.x, position.y)))
            }
            WindowEvent::MouseWheel { delta, .. } => match delta {
                MouseScrollDelta::LineDelta(dx, dy) => {
                    if dx == 0.0 && dy == 0.0 {
                        None
                    } else {
                        // TODO Also x?
                        Some(Event::MouseWheelScroll(
                            f64::from(dx),
                            scroll_wheel_multiplier() * f64::from(dy),
                        ))
                    }
                }
                // This one only happens on Mac. The scrolling is way too fast, so slow it down.
                // Probably the better way is to convert the LogicalPosition to a PhysicalPosition
                // somehow knowing the DPI.
                MouseScrollDelta::PixelDelta(pos) => {
                    Some(Event::MouseWheelScroll(0.1 * pos.x, 0.1 * pos.y))
                }
            },
            WindowEvent::Resized(size) => {
                Some(Event::WindowResized(size.width.into(), size.height.into()))
            }
            WindowEvent::Focused(gained) => Some(if gained {
                Event::WindowGainedCursor
            } else {
                Event::WindowLostCursor
            }),
            _ => None,
        }
    }
}

// For some reason, Y is inverted in the browser
#[cfg(feature = "wasm-backend")]
fn scroll_wheel_multiplier() -> f64 {
    -1.0
}

#[cfg(not(feature = "wasm-backend"))]
fn scroll_wheel_multiplier() -> f64 {
    1.0
}

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub enum Key {
    // Case is unspecified.
    // TODO Would be cool to represent A and UpperA, but then release semantics get weird... hold
    // shift and A, release shift -- does that trigger a Release(UpperA) and a Press(A)?
    A,
    B,
    C,
    D,
    E,
    F,
    G,
    H,
    I,
    J,
    K,
    L,
    M,
    N,
    O,
    P,
    Q,
    R,
    S,
    T,
    U,
    V,
    W,
    X,
    Y,
    Z,
    // Numbers (not the numpad)
    Num1,
    Num2,
    Num3,
    Num4,
    Num5,
    Num6,
    Num7,
    Num8,
    Num9,
    Num0,
    // symbols
    // TODO shift+number keys
    LeftBracket,
    RightBracket,
    Space,
    Slash,
    Dot,
    Comma,
    Semicolon,
    Colon,
    Equals,
    SingleQuote,
    // Stuff without a straightforward single-character display
    Escape,
    Enter,
    Tab,
    Backspace,
    LeftShift,
    LeftControl,
    RightControl,
    LeftAlt,
    RightAlt,
    LeftArrow,
    RightArrow,
    UpArrow,
    DownArrow,
    F1,
    F2,
    F3,
    F4,
    F5,
    F6,
    F7,
    F8,
    F9,
    F10,
    F11,
    F12,
}

impl Key {
    pub const NUM_KEYS: [Key; 9] = [
        Key::Num1,
        Key::Num2,
        Key::Num3,
        Key::Num4,
        Key::Num5,
        Key::Num6,
        Key::Num7,
        Key::Num8,
        Key::Num9,
    ];

    pub fn to_char(self, shift_pressed: bool) -> Option<char> {
        match self {
            Key::A => Some(if shift_pressed { 'A' } else { 'a' }),
            Key::B => Some(if shift_pressed { 'B' } else { 'b' }),
            Key::C => Some(if shift_pressed { 'C' } else { 'c' }),
            Key::D => Some(if shift_pressed { 'D' } else { 'd' }),
            Key::E => Some(if shift_pressed { 'E' } else { 'e' }),
            Key::F => Some(if shift_pressed { 'F' } else { 'f' }),
            Key::G => Some(if shift_pressed { 'G' } else { 'g' }),
            Key::H => Some(if shift_pressed { 'H' } else { 'h' }),
            Key::I => Some(if shift_pressed { 'I' } else { 'i' }),
            Key::J => Some(if shift_pressed { 'J' } else { 'j' }),
            Key::K => Some(if shift_pressed { 'K' } else { 'k' }),
            Key::L => Some(if shift_pressed { 'L' } else { 'l' }),
            Key::M => Some(if shift_pressed { 'M' } else { 'm' }),
            Key::N => Some(if shift_pressed { 'N' } else { 'n' }),
            Key::O => Some(if shift_pressed { 'O' } else { 'o' }),
            Key::P => Some(if shift_pressed { 'P' } else { 'p' }),
            Key::Q => Some(if shift_pressed { 'Q' } else { 'q' }),
            Key::R => Some(if shift_pressed { 'R' } else { 'r' }),
            Key::S => Some(if shift_pressed { 'S' } else { 's' }),
            Key::T => Some(if shift_pressed { 'T' } else { 't' }),
            Key::U => Some(if shift_pressed { 'U' } else { 'u' }),
            Key::V => Some(if shift_pressed { 'V' } else { 'v' }),
            Key::W => Some(if shift_pressed { 'W' } else { 'w' }),
            Key::X => Some(if shift_pressed { 'X' } else { 'x' }),
            Key::Y => Some(if shift_pressed { 'Y' } else { 'y' }),
            Key::Z => Some(if shift_pressed { 'Z' } else { 'z' }),
            Key::Num1 => Some(if shift_pressed { '!' } else { '1' }),
            Key::Num2 => Some(if shift_pressed { '@' } else { '2' }),
            Key::Num3 => Some(if shift_pressed { '#' } else { '3' }),
            Key::Num4 => Some(if shift_pressed { '$' } else { '4' }),
            Key::Num5 => Some(if shift_pressed { '%' } else { '5' }),
            Key::Num6 => Some(if shift_pressed { '^' } else { '6' }),
            Key::Num7 => Some(if shift_pressed { '&' } else { '7' }),
            Key::Num8 => Some(if shift_pressed { '*' } else { '8' }),
            Key::Num9 => Some(if shift_pressed { '(' } else { '9' }),
            Key::Num0 => Some(if shift_pressed { ')' } else { '0' }),
            Key::LeftBracket => Some(if shift_pressed { '{' } else { '[' }),
            Key::RightBracket => Some(if shift_pressed { '}' } else { ']' }),
            Key::Space => Some(' '),
            Key::Slash => Some(if shift_pressed { '?' } else { '/' }),
            Key::Dot => Some(if shift_pressed { '>' } else { '.' }),
            Key::Comma => Some(if shift_pressed { '<' } else { ',' }),
            Key::Semicolon => Some(';'),
            Key::Colon => Some(':'),
            Key::Equals => Some(if shift_pressed { '+' } else { '=' }),
            Key::SingleQuote => Some(if shift_pressed { '"' } else { '\'' }),
            Key::Escape
            | Key::Enter
            | Key::Tab
            | Key::Backspace
            | Key::LeftShift
            | Key::LeftControl
            | Key::RightControl
            | Key::LeftAlt
            | Key::RightAlt
            | Key::LeftArrow
            | Key::RightArrow
            | Key::UpArrow
            | Key::DownArrow
            | Key::F1
            | Key::F2
            | Key::F3
            | Key::F4
            | Key::F5
            | Key::F6
            | Key::F7
            | Key::F8
            | Key::F9
            | Key::F10
            | Key::F11
            | Key::F12 => None,
        }
    }

    pub fn describe(self) -> String {
        match self {
            Key::Escape => "Escape".to_string(),
            Key::Enter => "Enter".to_string(),
            Key::Tab => "Tab".to_string(),
            Key::Backspace => "Backspace".to_string(),
            Key::LeftShift => "Shift".to_string(),
            Key::LeftControl => "left Control".to_string(),
            Key::RightControl => "right Control".to_string(),
            Key::LeftAlt => "left Alt".to_string(),
            Key::RightAlt => "right Alt".to_string(),
            Key::LeftArrow => "← arrow".to_string(),
            Key::RightArrow => "→ arrow".to_string(),
            Key::UpArrow => "↑".to_string(),
            Key::DownArrow => "↓".to_string(),
            Key::F1 => "F1".to_string(),
            Key::F2 => "F2".to_string(),
            Key::F3 => "F3".to_string(),
            Key::F4 => "F4".to_string(),
            Key::F5 => "F5".to_string(),
            Key::F6 => "F6".to_string(),
            Key::F7 => "F7".to_string(),
            Key::F8 => "F8".to_string(),
            Key::F9 => "F9".to_string(),
            Key::F10 => "F10".to_string(),
            Key::F11 => "F11".to_string(),
            Key::F12 => "F12".to_string(),
            // These have to_char, but override here
            Key::Space => "Space".to_string(),
            _ => self.to_char(false).unwrap().to_string(),
        }
    }

    fn from_winit_key(input: KeyboardInput) -> Option<Key> {
        let key = input.virtual_keycode?;
        Some(match key {
            VirtualKeyCode::A => Key::A,
            VirtualKeyCode::B => Key::B,
            VirtualKeyCode::C => Key::C,
            VirtualKeyCode::D => Key::D,
            VirtualKeyCode::E => Key::E,
            VirtualKeyCode::F => Key::F,
            VirtualKeyCode::G => Key::G,
            VirtualKeyCode::H => Key::H,
            VirtualKeyCode::I => Key::I,
            VirtualKeyCode::J => Key::J,
            VirtualKeyCode::K => Key::K,
            VirtualKeyCode::L => Key::L,
            VirtualKeyCode::M => Key::M,
            VirtualKeyCode::N => Key::N,
            VirtualKeyCode::O => Key::O,
            VirtualKeyCode::P => Key::P,
            VirtualKeyCode::Q => Key::Q,
            VirtualKeyCode::R => Key::R,
            VirtualKeyCode::S => Key::S,
            VirtualKeyCode::T => Key::T,
            VirtualKeyCode::U => Key::U,
            VirtualKeyCode::V => Key::V,
            VirtualKeyCode::W => Key::W,
            VirtualKeyCode::X => Key::X,
            VirtualKeyCode::Y => Key::Y,
            VirtualKeyCode::Z => Key::Z,
            VirtualKeyCode::Key1 => Key::Num1,
            VirtualKeyCode::Key2 => Key::Num2,
            VirtualKeyCode::Key3 => Key::Num3,
            VirtualKeyCode::Key4 => Key::Num4,
            VirtualKeyCode::Key5 => Key::Num5,
            VirtualKeyCode::Key6 => Key::Num6,
            VirtualKeyCode::Key7 => Key::Num7,
            VirtualKeyCode::Key8 => Key::Num8,
            VirtualKeyCode::Key9 => Key::Num9,
            VirtualKeyCode::Key0 => Key::Num0,
            VirtualKeyCode::LBracket => Key::LeftBracket,
            VirtualKeyCode::RBracket => Key::RightBracket,
            VirtualKeyCode::Space => Key::Space,
            VirtualKeyCode::Slash => Key::Slash,
            VirtualKeyCode::Period => Key::Dot,
            VirtualKeyCode::Comma => Key::Comma,
            VirtualKeyCode::Semicolon => Key::Semicolon,
            VirtualKeyCode::Colon => Key::Colon,
            VirtualKeyCode::Equals => Key::Equals,
            VirtualKeyCode::Apostrophe => Key::SingleQuote,
            VirtualKeyCode::Escape => Key::Escape,
            VirtualKeyCode::Return => Key::Enter,
            VirtualKeyCode::Tab => Key::Tab,
            VirtualKeyCode::Back => Key::Backspace,
            VirtualKeyCode::LShift => Key::LeftShift,
            VirtualKeyCode::LControl => Key::LeftControl,
            VirtualKeyCode::RControl => Key::RightControl,
            VirtualKeyCode::LAlt => Key::LeftAlt,
            VirtualKeyCode::RAlt => Key::RightAlt,
            VirtualKeyCode::Left => Key::LeftArrow,
            VirtualKeyCode::Right => Key::RightArrow,
            VirtualKeyCode::Up => Key::UpArrow,
            VirtualKeyCode::Down => Key::DownArrow,
            VirtualKeyCode::F1 => Key::F1,
            VirtualKeyCode::F2 => Key::F2,
            VirtualKeyCode::F3 => Key::F3,
            VirtualKeyCode::F4 => Key::F4,
            VirtualKeyCode::F5 => Key::F5,
            VirtualKeyCode::F6 => Key::F6,
            VirtualKeyCode::F7 => Key::F7,
            VirtualKeyCode::F8 => Key::F8,
            VirtualKeyCode::F9 => Key::F9,
            VirtualKeyCode::F10 => Key::F10,
            VirtualKeyCode::F11 => Key::F11,
            VirtualKeyCode::F12 => Key::F12,
            _ => {
                println!("Unknown winit key {:?}", key);
                return None;
            }
        })
    }
}

// TODO This is not an ideal representation at all.
#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub enum MultiKey {
    Normal(Key),
    LCtrl(Key),
    Any(Vec<Key>),
}

impl MultiKey {
    pub fn describe(&self) -> String {
        match self {
            MultiKey::Normal(key) => key.describe(),
            MultiKey::LCtrl(key) => format!("Ctrl+{}", key.describe()),
            MultiKey::Any(ref keys) => keys
                .iter()
                .map(|k| k.describe())
                .collect::<Vec<_>>()
                .join(", "),
        }
    }
}

// For easy ModalMenu construction
pub fn hotkey(key: Key) -> Option<MultiKey> {
    Some(MultiKey::Normal(key))
}

pub fn lctrl(key: Key) -> Option<MultiKey> {
    Some(MultiKey::LCtrl(key))
}

pub fn hotkeys(keys: Vec<Key>) -> Option<MultiKey> {
    Some(MultiKey::Any(keys))
}
