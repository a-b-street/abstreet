use std::collections::BTreeSet;

use geom::ArrowCap;
use map_gui::render::{DrawOptions, DrawUberTurnGroup, BIG_ARROW_THICKNESS};
use map_model::{IntersectionCluster, IntersectionID};
use widgetry::{
    DrawBaselayer, EventCtx, GeomBatch, GfxCtx, HorizontalAlignment, Key, Outcome, Panel, State,
    StyledButtons, VerticalAlignment, Widget,
};

use crate::app::Transition;
use crate::app::{App, ShowEverything};

pub struct ClusterTrafficSignalEditor {
    panel: Panel,

    members: BTreeSet<IntersectionID>,
    groups: Vec<DrawUberTurnGroup>,
    group_selected: Option<usize>,
}

impl ClusterTrafficSignalEditor {
    pub fn new(ctx: &mut EventCtx, app: &mut App, ic: &IntersectionCluster) -> Box<dyn State<App>> {
        app.primary.current_selection = None;
        Box::new(ClusterTrafficSignalEditor {
            panel: Panel::new(Widget::row(vec![ctx
                .style()
                .btn_outline_light_text("Finish")
                .hotkey(Key::Escape)
                .build_def(ctx)]))
            .aligned(HorizontalAlignment::Center, VerticalAlignment::Top)
            .build(ctx),
            groups: DrawUberTurnGroup::new(ic, &app.primary.map),
            group_selected: None,
            members: ic.members.clone(),
        })
    }
}

impl State<App> for ClusterTrafficSignalEditor {
    fn event(&mut self, ctx: &mut EventCtx, _: &mut App) -> Transition {
        match self.panel.event(ctx) {
            Outcome::Clicked(x) => match x.as_ref() {
                "Finish" => {
                    return Transition::Pop;
                }
                _ => unreachable!(),
            },
            _ => {}
        }

        ctx.canvas_movement();
        if ctx.redo_mouseover() {
            self.group_selected = None;
            if let Some(pt) = ctx.canvas.get_cursor_in_map_space() {
                for (idx, g) in self.groups.iter().enumerate() {
                    if g.block.contains_pt(pt) {
                        self.group_selected = Some(idx);
                        break;
                    }
                }
            }
        }

        Transition::Keep
    }

    fn draw_baselayer(&self) -> DrawBaselayer {
        DrawBaselayer::Custom
    }

    fn draw(&self, g: &mut GfxCtx, app: &App) {
        {
            let mut opts = DrawOptions::new();
            opts.suppress_traffic_signal_details
                .extend(self.members.clone());
            app.draw(g, opts, &ShowEverything::new());
        }

        let mut batch = GeomBatch::new();
        for (idx, g) in self.groups.iter().enumerate() {
            if Some(idx) == self.group_selected {
                batch.push(app.cs.selected, g.block.clone());
                batch.push(
                    app.cs.selected,
                    g.group
                        .geom
                        .make_arrow(BIG_ARROW_THICKNESS, ArrowCap::Triangle),
                );
            } else {
                batch.push(app.cs.signal_turn_block_bg, g.block.clone());
            }
            let arrow_color = app.cs.signal_protected_turn;
            batch.push(arrow_color, g.arrow.clone());
        }
        batch.draw(g);

        self.panel.draw(g);
    }
}
