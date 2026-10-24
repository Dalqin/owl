//! UI-agnostic core: no printing, no prompting. Callers (CLI, future UI)
//! render the structured data returned here.

pub mod apply;
pub mod cfg;
pub mod diff;
pub mod fomod;
pub mod images;
pub mod library;
pub mod locate;
pub mod manifest;
pub mod notes;
pub mod order_fix;
pub mod plugin;
pub mod plugin_order;
pub mod profile;
pub mod scan;
pub mod schemas;
pub mod validate;
