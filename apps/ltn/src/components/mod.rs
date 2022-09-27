mod about;
mod appwide_panel;
mod layers;
mod left_panel;
mod top_panel;

pub use appwide_panel::AppwidePanel;
pub use layers::Layers;
pub use left_panel::LeftPanel;
pub use top_panel::TopPanel;

#[derive(Clone, Copy, PartialEq)]
pub enum Mode {
    PickArea,
    ModifyNeighbourhood,
    SelectBoundary,
    RoutePlanner,
    Impact,
}
