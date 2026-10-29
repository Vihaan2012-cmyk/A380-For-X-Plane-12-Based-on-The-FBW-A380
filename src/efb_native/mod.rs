//! The EFB without Chromium: page trees from `efb_pages`, styled by
//! css_cascade with the EFB's own stylesheet, published to the `efbx`
//! display (`a380.efb.dom`), with input handled here. See
//! docs/superpowers/plans/2026-10-22-native-efb-host.md.

pub mod data;
pub mod doc;
pub mod engine;
pub mod ground_eng;
pub mod hit;
pub mod pdf;
pub mod scroll;
pub mod style;
pub mod text;
#[cfg(feature = "native-displays")]
pub mod host;
#[cfg(test)]
mod tests;
