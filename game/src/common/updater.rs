use std::collections::BTreeMap;

use abstutil::{DataPacks, Manifest};
use widgetry::{Btn, Checkbox, EventCtx, GfxCtx, Line, Outcome, Panel, State, TextExt, Widget};

use crate::app::App;
use crate::game::Transition;

pub struct Picker {
    panel: Panel,
}

impl Picker {
    pub fn new(ctx: &mut EventCtx) -> Box<dyn State<App>> {
        let manifest = Manifest::load();
        let data_packs = DataPacks::load_or_create();

        let mut col = vec![Widget::row(vec![
            Line("Download more cities").small_heading().draw(ctx),
            Btn::close(ctx),
        ])];
        for (city, bytes) in size_per_city(&manifest) {
            col.push(Widget::row(vec![
                Checkbox::checkbox(ctx, &city, None, data_packs.runtime.contains(&city)),
                prettyprint_bytes(bytes).draw_text(ctx),
            ]));
        }

        Box::new(Picker {
            panel: Panel::new(Widget::col(col)).build(ctx),
        })
    }
}

impl State<App> for Picker {
    fn event(&mut self, ctx: &mut EventCtx, _: &mut App) -> Transition {
        match self.panel.event(ctx) {
            Outcome::Clicked(x) => match x.as_ref() {
                "close" => {
                    return Transition::Pop;
                }
                _ => unreachable!(),
            },
            _ => {}
        }

        Transition::Keep
    }

    fn draw(&self, g: &mut GfxCtx, _: &App) {
        self.panel.draw(g);
    }
}

// For each city, how many total bytes do the runtime files cost?
fn size_per_city(manifest: &Manifest) -> BTreeMap<String, usize> {
    let mut per_city = BTreeMap::new();
    for (path, entry) in &manifest.entries {
        let parts = path.split("/").collect::<Vec<_>>();
        if parts[1] == "system" {
            *per_city.entry(parts[2].to_string()).or_insert(0) += entry.size_bytes;
        }
    }
    per_city
}

fn prettyprint_bytes(bytes: usize) -> String {
    if bytes < 1024 {
        return format!("{} bytes", bytes);
    }
    let kb = (bytes as f64) / 1024.0;
    if kb < 1024.0 {
        return format!("{} kb", kb as usize);
    }
    let mb = kb / 1024.0;
    format!("{} mb", mb as usize)
}
