use crate::ScreenPt;
use piston::input as pi;

#[derive(Clone, Copy, PartialEq)]
pub enum Event {
    // TODO Get rid of this after handling all cases.
    Unknown,
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
    // Vertical only
    MouseWheelScroll(f64),
    WindowResized(f64, f64),
}

impl Event {
    pub fn from_piston_event(ev: pi::Event) -> Event {
        use piston::input::{
            ButtonEvent, MouseCursorEvent, MouseScrollEvent, PressEvent, ReleaseEvent, ResizeEvent,
            TouchEvent, UpdateEvent,
        };

        if let Some(pi::Button::Mouse(button)) = ev.press_args() {
            if button == pi::MouseButton::Left {
                return Event::LeftMouseButtonDown;
            }
            if button == pi::MouseButton::Right {
                return Event::RightMouseButtonDown;
            }
        }
        if let Some(pi::Button::Mouse(button)) = ev.release_args() {
            if button == pi::MouseButton::Left {
                return Event::LeftMouseButtonUp;
            }
            if button == pi::MouseButton::Right {
                return Event::RightMouseButtonUp;
            }
        }

        if let Some(pi::Button::Keyboard(key)) = ev.press_args() {
            if let Some(key) = Key::from_piston_key(key, ev.button_args()) {
                return Event::KeyPress(key);
            }
            return Event::Unknown;
        }
        if let Some(pi::Button::Keyboard(key)) = ev.release_args() {
            if let Some(key) = Key::from_piston_key(key, ev.button_args()) {
                return Event::KeyRelease(key);
            }
            return Event::Unknown;
        }

        if ev.update_args().is_some() {
            return Event::Update;
        }
        if let Some(pair) = ev.mouse_cursor_args() {
            return Event::MouseMovedTo(ScreenPt::new(pair[0], pair[1]));
        }
        if let Some(args) = ev.touch_args() {
            // The docs say these are normalized [0, 1] coordinates, but... they're not. :D
            return Event::MouseMovedTo(ScreenPt::new(args.x, args.y));
        }
        if let Some(pair) = ev.mouse_scroll_args() {
            return Event::MouseWheelScroll(pair[1]);
        }
        if let Some(pair) = ev.resize_args() {
            return Event::WindowResized(f64::from(pair[0]), f64::from(pair[1]));
        }

        panic!("Unknown piston event {:?}", ev);
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
    // Stuff without a straightforward single-character display
    Escape,
    Enter,
    Tab,
    Backspace,
    LeftShift,
    LeftControl,
    LeftAlt,
    LeftArrow,
    RightArrow,
    UpArrow,
    DownArrow,
}

impl Key {
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
            Key::Semicolon => Some(if shift_pressed { ':' } else { ';' }),
            Key::Escape
            | Key::Enter
            | Key::Tab
            | Key::Backspace
            | Key::LeftShift
            | Key::LeftControl
            | Key::LeftAlt
            | Key::LeftArrow
            | Key::RightArrow
            | Key::UpArrow
            | Key::DownArrow => None,
        }
    }

    pub fn describe(self: Key) -> String {
        match self {
            Key::Escape => "Escape".to_string(),
            Key::Enter => "Enter".to_string(),
            Key::Tab => "Tab".to_string(),
            Key::Backspace => "Backspace".to_string(),
            Key::LeftShift => "Shift".to_string(),
            Key::LeftControl => "Control".to_string(),
            Key::LeftAlt => "Alt".to_string(),
            Key::LeftArrow => "Left arrow key".to_string(),
            Key::RightArrow => "Right arrow key".to_string(),
            Key::UpArrow => "Up arrow key".to_string(),
            Key::DownArrow => "Down arrow key".to_string(),
            // These have to_char, but override here
            Key::Space => "Space".to_string(),
            _ => self.to_char(false).unwrap().to_string(),
        }
    }

    fn from_piston_key(key: pi::Key, args: Option<pi::ButtonArgs>) -> Option<Key> {
        if let Some(a) = args {
            if a.scancode == Some(39) {
                return Some(Key::Semicolon);
            }
        }

        Some(match key {
            pi::Key::A => Key::A,
            pi::Key::B => Key::B,
            pi::Key::C => Key::C,
            pi::Key::D => Key::D,
            pi::Key::E => Key::E,
            pi::Key::F => Key::F,
            pi::Key::G => Key::G,
            pi::Key::H => Key::H,
            pi::Key::I => Key::I,
            pi::Key::J => Key::J,
            pi::Key::K => Key::K,
            pi::Key::L => Key::L,
            pi::Key::M => Key::M,
            pi::Key::N => Key::N,
            pi::Key::O => Key::O,
            pi::Key::P => Key::P,
            pi::Key::Q => Key::Q,
            pi::Key::R => Key::R,
            pi::Key::S => Key::S,
            pi::Key::T => Key::T,
            pi::Key::U => Key::U,
            pi::Key::V => Key::V,
            pi::Key::W => Key::W,
            pi::Key::X => Key::X,
            pi::Key::Y => Key::Y,
            pi::Key::Z => Key::Z,
            pi::Key::D1 => Key::Num1,
            pi::Key::D2 => Key::Num2,
            pi::Key::D3 => Key::Num3,
            pi::Key::D4 => Key::Num4,
            pi::Key::D5 => Key::Num5,
            pi::Key::D6 => Key::Num6,
            pi::Key::D7 => Key::Num7,
            pi::Key::D8 => Key::Num8,
            pi::Key::D9 => Key::Num9,
            pi::Key::D0 => Key::Num0,
            pi::Key::LeftBracket => Key::LeftBracket,
            pi::Key::RightBracket => Key::RightBracket,
            pi::Key::Space => Key::Space,
            pi::Key::Slash => Key::Slash,
            pi::Key::Period => Key::Dot,
            pi::Key::Comma => Key::Comma,
            pi::Key::Escape => Key::Escape,
            pi::Key::Return => Key::Enter,
            pi::Key::Tab => Key::Tab,
            pi::Key::Backspace => Key::Backspace,
            pi::Key::LShift => Key::LeftShift,
            pi::Key::LCtrl => Key::LeftControl,
            pi::Key::LAlt => Key::LeftAlt,
            pi::Key::Left => Key::LeftArrow,
            pi::Key::Right => Key::RightArrow,
            pi::Key::Up => Key::UpArrow,
            pi::Key::Down => Key::DownArrow,
            _ => {
                println!("Unknown piston key {:?}", key);
                return None;
            }
        })
    }
}
