//! Ferrocast core: platform-independent logic (search, ranking, calculator,
//! placeholders, script commands, persisted models).

pub mod ai;
pub mod annotate;
pub mod calc;
pub mod chatgpt;
pub mod clipping;
pub mod collage;
pub use clipping::clipping_fixture_rsrc;
pub mod color;
pub mod convert;
pub mod currency;
pub mod epub;
pub mod epub_write;
pub mod frame;
pub mod fuzzy;
pub mod import;
pub mod launch;
pub mod markdown;
pub mod matrix;
pub mod media;
pub mod models;
pub mod pages;
pub mod pdfplan;
pub mod photo;
pub mod placeholders;
pub mod ranking;
pub mod redact;
pub mod script;
pub mod storage;
pub mod subs;
pub mod timer;
pub mod timezones;
pub mod units;
pub mod wheel;
pub mod xlsx;
