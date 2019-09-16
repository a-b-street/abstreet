// TODO None of this works yet.

use crate::{Color, MultiKey, ScreenDims, ScreenPt, Text};

// TODO This is the concrete state at some time. When something changes, how do we recalculate it?
struct Panel {
    rows: Vec<Row>,
    top_left: ScreenPt,
    bg: Option<Color>,
}

struct Row {
    total_width: f64,
    total_height: f64,
    bg: Option<Color>,
    items: Vec<Item>,
}

enum Item {
    Text(Text),
    Button(Option<MultiKey>, String),
    HideIcon,
    UnhideIcon,
    Spacer(ScreenDims),
}

fn modal_menu_ex() {
    let mut panel = Panel {
        rows: vec![
            Row {
                bg: Some(Color::BLUE),
                items: vec![Item::Text(Text::from(Line("title").fg(Color::WHITE))), Item::HideIcon],
            },
            Row {
                bg: None,
                items: vec![Item::Button(hotkey(Key::L), "load thingy")],
            }
            Row {
                bg: None,
                items: vec![Item::Button(hotkey(Key::M), "manage thingy")],
            }
            Row {
                bg: Some(Color::BLACK),
                // TODO really the spacer should expand to full width...
                items: vec![Item::Spacer(ScreenDims::new(1.0, 30.0))],
            },
            Row {
                bg: None,
                items: vec![Item::Button(None, "complex action")],
            }
        ],
        bg: Some(Color::grey(0.5)),
    };
}

// TODO how could scrolling be built on top of this?
