//! MathML layout for HTML/CSS engines and standalone use.
//!
//! - [`host`]: lay out math on behalf of a host layout engine whose CSS is the source of truth.
//! - [`standalone`]: a minimal MathML document with a built-in user agent stylesheet.
//! - [`mathmlparser`]: the operator dictionary and (with the `mathml_parser` feature) an XML
//!   parser that produces a [`standalone::MathDocument`].
//! - `svg_renderer` (feature `svg`): draws a laid-out document as SVG.
//! - [`speech`]: spoken text for screen readers, built from the MathML structure.
//! - [`shaper`]: [`shaper::Shaper`], a [`host::MathFont`] based on rustybuzz.

pub mod host;
pub mod mathmlparser;
pub mod speech;
pub mod standalone;
#[cfg(feature = "svg")]
pub mod svg_renderer;
mod typesetting;

pub use crate::standalone::layout;
pub use crate::typesetting::{shaper, unicode_math};
