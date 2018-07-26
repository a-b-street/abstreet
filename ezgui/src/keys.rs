use piston::input::Key;

pub(crate) fn describe_key(key: Key) -> String {
    match key {
        Key::Space => "Space".to_string(),
        Key::Escape => "Escape".to_string(),
        Key::Return => "Enter".to_string(),
        Key::Tab => "Tab".to_string(),
        Key::Backspace => "Backspace".to_string(),
        _ => {
            if let Some(c) = key_to_char(key) {
                return c.to_string();
            }
            format!("{:?}", key)
        }
    }
}

// Returns uppercase form
pub(crate) fn key_to_char(key: Key) -> Option<char> {
    match key {
        Key::Space => Some(' '),
        Key::A => Some('A'),
        Key::B => Some('B'),
        Key::C => Some('C'),
        Key::D => Some('D'),
        Key::E => Some('E'),
        Key::F => Some('F'),
        Key::G => Some('G'),
        Key::H => Some('H'),
        Key::I => Some('I'),
        Key::J => Some('J'),
        Key::K => Some('K'),
        Key::L => Some('L'),
        Key::M => Some('M'),
        Key::N => Some('N'),
        Key::O => Some('O'),
        Key::P => Some('P'),
        Key::Q => Some('Q'),
        Key::R => Some('R'),
        Key::S => Some('S'),
        Key::T => Some('T'),
        Key::U => Some('U'),
        Key::V => Some('V'),
        Key::W => Some('W'),
        Key::X => Some('X'),
        Key::Y => Some('Y'),
        Key::Z => Some('Z'),
        Key::D0 => Some('0'),
        Key::D1 => Some('1'),
        Key::D2 => Some('2'),
        Key::D3 => Some('3'),
        Key::D4 => Some('4'),
        Key::D5 => Some('5'),
        Key::D6 => Some('6'),
        Key::D7 => Some('7'),
        Key::D8 => Some('8'),
        Key::D9 => Some('9'),
        Key::Slash => Some('/'),
        Key::LeftBracket => Some('['),
        Key::RightBracket => Some(']'),
        _ => None,
    }
}
