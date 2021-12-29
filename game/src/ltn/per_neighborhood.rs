use map_gui::tools::CityPicker;
use widgetry::{
    EventCtx, HorizontalAlignment, Key, Panel, State, VerticalAlignment, Widget,
    DEFAULT_CORNER_RADIUS,
};

use super::{BrowseNeighborhoods, Neighborhood};
use crate::app::{App, Transition};

#[derive(PartialEq)]
pub enum Tab {
    Connectivity,
    RatRuns,
    Pathfinding,
}

pub trait TakeNeighborhood {
    fn take_neighborhood(self) -> Neighborhood;
}

impl Tab {
    pub fn make_panel(self, ctx: &mut EventCtx, app: &App, per_tab_contents: Widget) -> Panel {
        Panel::new_builder(Widget::col(vec![
            map_gui::tools::app_header(ctx, app, "Low traffic neighborhoods"),
            Widget::row(vec![
                ctx.style()
                    .btn_outline
                    .text("Browse neighborhoods")
                    .hotkey(Key::Escape)
                    .build_def(ctx),
                ctx.style()
                    .btn_outline
                    .text("Adjust boundary")
                    .hotkey(Key::B)
                    .build_def(ctx),
            ]),
            self.make_buttons(ctx),
            per_tab_contents.tab_body(ctx),
        ]))
        .aligned(HorizontalAlignment::Left, VerticalAlignment::Top)
        .build(ctx)
    }

    pub fn must_handle_action<T: TakeNeighborhood + State<App>>(
        self,
        ctx: &mut EventCtx,
        app: &mut App,
        action: &str,
    ) -> Transition {
        match action {
            "Home" => Transition::Clear(vec![crate::pregame::TitleScreen::new_state(ctx, app)]),
            "change map" => Transition::Push(CityPicker::new_state(
                ctx,
                app,
                Box::new(|ctx, app| Transition::Replace(BrowseNeighborhoods::new_state(ctx, app))),
            )),
            "Browse neighborhoods" => Transition::Pop,
            "Adjust boundary" => Transition::ConsumeState(Box::new(|state, ctx, app| {
                let state = state.downcast::<T>().ok().unwrap();
                vec![super::select_boundary::SelectBoundary::new_state(
                    ctx,
                    app,
                    Some(state.take_neighborhood().orig_perimeter),
                )]
            })),
            "Connectivity" => Transition::ConsumeState(Box::new(|state, ctx, app| {
                let state = state.downcast::<T>().ok().unwrap();
                vec![super::connectivity::Viewer::new_state(
                    ctx,
                    app,
                    state.take_neighborhood(),
                )]
            })),
            "Rat runs" => Transition::ConsumeState(Box::new(|state, ctx, app| {
                let state = state.downcast::<T>().ok().unwrap();
                vec![super::rat_run_viewer::BrowseRatRuns::new_state(
                    ctx,
                    app,
                    state.take_neighborhood(),
                )]
            })),
            "Pathfinding" => Transition::ConsumeState(Box::new(|state, ctx, app| {
                let state = state.downcast::<T>().ok().unwrap();
                vec![super::route::RoutePlanner::new_state(
                    ctx,
                    app,
                    state.take_neighborhood(),
                )]
            })),
            _ => unreachable!(),
        }
    }

    fn make_buttons(self, ctx: &mut EventCtx) -> Widget {
        let mut row = Vec::new();
        for (tab, label, key) in [
            (Tab::Connectivity, "Connectivity", Key::Num1),
            (Tab::RatRuns, "Rat runs", Key::Num2),
            (Tab::Pathfinding, "Pathfinding", Key::Num3),
        ] {
            // TODO Match the TabController styling
            row.push(
                ctx.style()
                    .btn_tab
                    .text(label)
                    .corner_rounding(geom::CornerRadii {
                        top_left: DEFAULT_CORNER_RADIUS,
                        top_right: DEFAULT_CORNER_RADIUS,
                        bottom_left: 0.0,
                        bottom_right: 0.0,
                    })
                    .hotkey(key)
                    // We abuse "disabled" to denote "currently selected"
                    .disabled(self == tab)
                    .build_def(ctx),
            );
        }
        Widget::row(row)
    }
}
