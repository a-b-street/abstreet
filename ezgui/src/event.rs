use crate::ScreenPt;
use glium::glutin;

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
    // Time has passed; EventLoopMode::Animation is active
    Update,
    MouseMovedTo(ScreenPt),
    WindowLostCursor,
    WindowGainedCursor,
    // Vertical only
    MouseWheelScroll(f64),
    WindowResized(f64, f64),
}

impl Event {
    pub fn from_glutin_event(ev: glutin::WindowEvent, hidpi_factor: f64) -> Option<Event> {
        match ev {
            glutin::WindowEvent::MouseInput { state, button, .. } => match (button, state) {
                (glutin::MouseButton::Left, glutin::ElementState::Pressed) => {
                    Some(Event::LeftMouseButtonDown)
                }
                (glutin::MouseButton::Left, glutin::ElementState::Released) => {
                    Some(Event::LeftMouseButtonUp)
                }
                (glutin::MouseButton::Right, glutin::ElementState::Pressed) => {
                    Some(Event::RightMouseButtonDown)
                }
                (glutin::MouseButton::Right, glutin::ElementState::Released) => {
                    Some(Event::RightMouseButtonUp)
                }
                _ => None,
            },
            glutin::WindowEvent::KeyboardInput { input, .. } => {
                if let Some(key) = Key::from_glutin_key(input) {
                    if input.state == glutin::ElementState::Pressed {
                        Some(Event::KeyPress(key))
                    } else {
                        Some(Event::KeyRelease(key))
                    }
                } else {
                    None
                }
            }
            glutin::WindowEvent::CursorMoved { position, .. } => {
                let pos = position.to_physical(hidpi_factor);
                Some(Event::MouseMovedTo(ScreenPt::new(pos.x, pos.y)))
            }
            glutin::WindowEvent::MouseWheel { delta, .. } => match delta {
                glutin::MouseScrollDelta::LineDelta(_, dy) => {
                    if dy == 0.0 {
                        None
                    } else {
                        Some(Event::MouseWheelScroll(f64::from(dy)))
                    }
                }
                // This one only happens on Mac. The scrolling is way too fast, so slow it down.
                // Probably the better way is to convert the LogicalPosition to a PhysicalPosition
                // somehow knowing the DPI.
                glutin::MouseScrollDelta::PixelDelta(pos) => {
                    Some(Event::MouseWheelScroll(0.1 * pos.y))
                }
            },
            glutin::WindowEvent::Resized(size) => {
                let actual_size = glutin::dpi::PhysicalSize::from_logical(size, hidpi_factor);
                Some(Event::WindowResized(actual_size.width, actual_size.height))
            }
            glutin::WindowEvent::Focused(gained) => Some(if gained {
                Event::WindowGainedCursor
            } else {
                Event::WindowLostCursor
            }),
            _ => None,
        }
    }
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

    fn from_glutin_key(input: glutin::KeyboardInput) -> Option<Key> {
        let key = input.virtual_keycode?;
        Some(match key {
            glutin::VirtualKeyCode::A => Key::A,
            glutin::VirtualKeyCode::B => Key::B,
            glutin::VirtualKeyCode::C => Key::C,
            glutin::VirtualKeyCode::D => Key::D,
            glutin::VirtualKeyCode::E => Key::E,
            glutin::VirtualKeyCode::F => Key::F,
            glutin::VirtualKeyCode::G => Key::G,
            glutin::VirtualKeyCode::H => Key::H,
            glutin::VirtualKeyCode::I => Key::I,
            glutin::VirtualKeyCode::J => Key::J,
            glutin::VirtualKeyCode::K => Key::K,
            glutin::VirtualKeyCode::L => Key::L,
            glutin::VirtualKeyCode::M => Key::M,
            glutin::VirtualKeyCode::N => Key::N,
            glutin::VirtualKeyCode::O => Key::O,
            glutin::VirtualKeyCode::P => Key::P,
            glutin::VirtualKeyCode::Q => Key::Q,
            glutin::VirtualKeyCode::R => Key::R,
            glutin::VirtualKeyCode::S => Key::S,
            glutin::VirtualKeyCode::T => Key::T,
            glutin::VirtualKeyCode::U => Key::U,
            glutin::VirtualKeyCode::V => Key::V,
            glutin::VirtualKeyCode::W => Key::W,
            glutin::VirtualKeyCode::X => Key::X,
            glutin::VirtualKeyCode::Y => Key::Y,
            glutin::VirtualKeyCode::Z => Key::Z,
            glutin::VirtualKeyCode::Key1 => Key::Num1,
            glutin::VirtualKeyCode::Key2 => Key::Num2,
            glutin::VirtualKeyCode::Key3 => Key::Num3,
            glutin::VirtualKeyCode::Key4 => Key::Num4,
            glutin::VirtualKeyCode::Key5 => Key::Num5,
            glutin::VirtualKeyCode::Key6 => Key::Num6,
            glutin::VirtualKeyCode::Key7 => Key::Num7,
            glutin::VirtualKeyCode::Key8 => Key::Num8,
            glutin::VirtualKeyCode::Key9 => Key::Num9,
            glutin::VirtualKeyCode::Key0 => Key::Num0,
            glutin::VirtualKeyCode::LBracket => Key::LeftBracket,
            glutin::VirtualKeyCode::RBracket => Key::RightBracket,
            glutin::VirtualKeyCode::Space => Key::Space,
            glutin::VirtualKeyCode::Slash => Key::Slash,
            glutin::VirtualKeyCode::Period => Key::Dot,
            glutin::VirtualKeyCode::Comma => Key::Comma,
            glutin::VirtualKeyCode::Semicolon => Key::Semicolon,
            glutin::VirtualKeyCode::Colon => Key::Colon,
            glutin::VirtualKeyCode::Equals => Key::Equals,
            glutin::VirtualKeyCode::Apostrophe => Key::SingleQuote,
            glutin::VirtualKeyCode::Escape => Key::Escape,
            glutin::VirtualKeyCode::Return => Key::Enter,
            glutin::VirtualKeyCode::Tab => Key::Tab,
            glutin::VirtualKeyCode::Back => Key::Backspace,
            glutin::VirtualKeyCode::LShift => Key::LeftShift,
            glutin::VirtualKeyCode::LControl => Key::LeftControl,
            glutin::VirtualKeyCode::RControl => Key::RightControl,
            glutin::VirtualKeyCode::LAlt => Key::LeftAlt,
            glutin::VirtualKeyCode::RAlt => Key::RightAlt,
            glutin::VirtualKeyCode::Left => Key::LeftArrow,
            glutin::VirtualKeyCode::Right => Key::RightArrow,
            glutin::VirtualKeyCode::Up => Key::UpArrow,
            glutin::VirtualKeyCode::Down => Key::DownArrow,
            glutin::VirtualKeyCode::F1 => Key::F1,
            glutin::VirtualKeyCode::F2 => Key::F2,
            glutin::VirtualKeyCode::F3 => Key::F3,
            glutin::VirtualKeyCode::F4 => Key::F4,
            glutin::VirtualKeyCode::F5 => Key::F5,
            glutin::VirtualKeyCode::F6 => Key::F6,
            glutin::VirtualKeyCode::F7 => Key::F7,
            glutin::VirtualKeyCode::F8 => Key::F8,
            glutin::VirtualKeyCode::F9 => Key::F9,
            glutin::VirtualKeyCode::F10 => Key::F10,
            glutin::VirtualKeyCode::F11 => Key::F11,
            glutin::VirtualKeyCode::F12 => Key::F12,
            _ => {
                println!("Unknown glutin key {:?}", key);
                return None;
            }
        })
    }
}

// TODO This is not an ideal representation at all.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub struct MultiKey {
    pub key: Key,
    pub lctrl: bool,
}

impl MultiKey {
    pub fn describe(self) -> String {
        if self.lctrl {
            format!("Ctrl+{}", self.key.describe())
        } else {
            self.key.describe()
        }
    }
}

// For easy ModalMenu construction
pub fn hotkey(key: Key) -> Option<MultiKey> {
    Some(MultiKey { key, lctrl: false })
}

pub fn lctrl(key: Key) -> Option<MultiKey> {
    Some(MultiKey { key, lctrl: true })
}
