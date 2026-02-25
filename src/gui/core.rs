#[path = "core/config_io.rs"]
mod config_io;
#[path = "core/deck_layout.rs"]
mod deck_layout;
#[path = "core/editor.rs"]
mod editor;
#[path = "core/icon_cache.rs"]
mod icon_cache;
#[path = "core/icon_catalog.rs"]
mod icon_catalog;
#[path = "core/paging.rs"]
mod paging;
#[path = "core/prelude.rs"]
mod prelude;
#[path = "core/style.rs"]
mod style;

pub(crate) use config_io::*;
pub(crate) use deck_layout::*;
pub(crate) use editor::*;
pub(crate) use icon_cache::*;
pub(crate) use icon_catalog::*;
pub(crate) use paging::*;
pub(crate) use prelude::*;
pub(crate) use style::*;
