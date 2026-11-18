//! Appolis, FocusMap's living city view.
//!
//! The city is rendered with wgpu inside an egui paint callback. Simulation
//! stays on its own worker thread; this crate only exposes the renderer and
//! the data types the app feeds it.

mod gpu;
mod scene;

pub use gpu::{pick, CityLook, CityPreview, CityRenderer};
pub use scene::{
    layout, Building, CityApp, CitySnapshot, District, Mood, MAX_BUILDINGS,
};
