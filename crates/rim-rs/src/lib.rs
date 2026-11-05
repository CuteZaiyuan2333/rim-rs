//! `rim-rs` — a builder-based RimWorld 1.6 mod generator.
//!
//! The crate is intentionally small: it models XML, Defs, patches, C# source
//! and assets as Rust values, then expands them into a ready-to-load mod
//! directory (and optionally compiles C# with the local `dotnet`).
//!
//! The primary entry point is [`ModBuilder`],
//! re-exported as `Mod` in [`prelude`].

pub mod assets;
pub mod builder;
pub mod csharp;
pub mod defs;
pub mod dotnet;
pub mod error;
pub mod game;
pub mod package;
pub mod patch;
pub mod prelude;
pub mod project;
pub mod xml;

pub use builder::{BuildOptions, BuildReport, ModBuilder};
pub use error::{Error, Result};
