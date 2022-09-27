mod about;
mod layers;
mod left_panel;
mod top_panel;

pub use layers::Layers;
pub use left_panel::LeftPanel;
pub use top_panel::{FilePanel, TopPanel};

#[derive(PartialEq)]
pub enum Mode {
    PickArea,
    ModifyNeighbourhood,
    SelectBoundary,
    RoutePlanner,
    Impact,
}
