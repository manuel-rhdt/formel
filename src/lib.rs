//! MathML layout for HTML/CSS engines and standalone use.
//!
//! - [`host`]: lay out math on behalf of a host layout engine whose CSS is the source of truth.
//! - [`standalone`]: a minimal MathML document with a built-in user agent stylesheet.
//! - [`mathmlparser`]: the operator dictionary and (with the `mathml_parser` feature) an XML
//!   parser that produces a [`standalone::MathDocument`].
//! - [`shaper`]: [`shaper::HarfbuzzShaper`], a [`host::MathFont`] based on HarfBuzz.

pub mod host;
pub mod mathmlparser;
pub mod standalone;
mod typesetting;

pub use crate::standalone::layout;
pub use crate::typesetting::{shaper, unicode_math};
