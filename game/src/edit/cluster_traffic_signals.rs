use crate::app::{App, ShowEverything};
use crate::game::{DrawBaselayer, State, Transition};
use crate::render::{DrawOptions, DrawUberTurnGroup, BIG_ARROW_THICKNESS};
use ezgui::{
    hotkey, Btn, Composite, EventCtx, GeomBatch, GfxCtx, HorizontalAlignment, Key, Outcome,
    VerticalAlignment, Widget,
};
use geom::ArrowCap;
use map_model::{IntersectionCluster, IntersectionID};
use std::collections::BTreeSet;

pub struct ClusterTrafficSignalEditor {
    composite: Composite,

    members: BTreeSet<IntersectionID>,
    groups: Vec<DrawUberTurnGroup>,
    group_selected: Option<usize>,
}

impl ClusterTrafficSignalEditor {
    pub fn new(ctx: &mut EventCtx, app: &mut App, ic: &IntersectionCluster) -> Box<dyn State> {
        app.primary.current_selection = None;
        Box::new(ClusterTrafficSignalEditor {
            composite: Composite::new(
                Widget::row(vec![
                    Btn::text_fg("Finish").build_def(ctx, hotkey(Key::Escape))
                ])
                .bg(app.cs.panel_bg),
            )
            .aligned(HorizontalAlignment::Center, VerticalAlignment::Top)
            .build(ctx),
            groups: DrawUberTurnGroup::new(ic, &app.primary.map),
            group_selected: None,
            members: ic.members.clone(),
        })
    }
}

impl State for ClusterTrafficSignalEditor {
    fn event(&mut self, ctx: &mut EventCtx, _: &mut App) -> Transition {
        match self.composite.event(ctx) {
            Some(Outcome::Clicked(x)) => match x.as_ref() {
                "Finish" => {
                    return Transition::Pop;
                }
                _ => unreachable!(),
            },
            None => {}
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
            app.draw(g, opts, &app.primary.sim, &ShowEverything::new());
        }

        let mut batch = GeomBatch::new();
        for (idx, g) in self.groups.iter().enumerate() {
            if Some(idx) == self.group_selected {
                batch.push(app.cs.selected, g.block.clone());
                batch.push(
                    app.cs.selected,
                    g.group
                        .geom
                        .make_arrow(BIG_ARROW_THICKNESS, ArrowCap::Triangle)
                        .unwrap(),
                );
            } else {
                batch.push(app.cs.signal_turn_block_bg, g.block.clone());
            }
            let arrow_color = app.cs.signal_protected_turn;
            batch.push(arrow_color, g.arrow.clone());
        }
        batch.draw(g);

        self.composite.draw(g);
    }
}
