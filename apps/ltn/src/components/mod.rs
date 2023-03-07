mod appwide_panel;
mod layers;
mod left_panel;

pub use appwide_panel::AppwidePanel;
pub use layers::{legend_entry, Layers};
pub use left_panel::{BottomPanel, LeftPanel};

#[derive(Clone, Copy, PartialEq)]
pub enum Mode {
    PickArea,
    FreehandBoundary,
    ModifyNeighbourhood,
    SelectBoundary,
    PerResidentImpact,
    RoutePlanner,
    Crossings,
    Impact,
    CycleNetwork,
}
