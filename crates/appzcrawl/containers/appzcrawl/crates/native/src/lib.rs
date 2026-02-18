#![deny(clippy::all)]

pub use crate::crawler::*;
pub use crate::engpicker::*;
pub use crate::html::*;
#[cfg(feature = "pdf")]
pub use crate::pdf::*;
pub use crate::utils::*;

pub use crate::document::{DocumentConverter, DocumentType};

mod crawler;
mod document;
mod engpicker;
mod html;
#[cfg(feature = "pdf")]
mod pdf;
mod utils;

pub use serde::{Deserialize, Serialize};
