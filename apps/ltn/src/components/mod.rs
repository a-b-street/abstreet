mod about;
mod layers;
mod left_panel;
mod top_panel;

pub use layers::Layers;
pub use left_panel::LeftPanel;
pub use top_panel::TopPanel;

#[derive(PartialEq)]
pub enum Mode {
    BrowseNeighbourhoods,
    ModifyNeighbourhood,
    SelectBoundary,
    RoutePlanner,
    Impact,
}
