//! Convenient imports for building a mod.
//!
//! ```no_run
//! use rim_rs::prelude::*;
//!
//! # fn main() -> rim_rs::Result<()> {
//! Mod::builder("Hello Rim")
//!     .package_id("me.hello")
//!     .defs(|defs| {
//!         defs.add(ThingDef::new("HelloThing").parent("BaseThing"));
//!     })
//!     .build()?;
//! # Ok(())
//! # }
//! ```

pub use crate::Result;
pub use crate::assets::AssetSet;
pub use crate::builder::{BuildOptions, BuildReport, ModBuilder, ModBuilder as Mod};
pub use crate::defs::{Def, DefSet, IntoDef, RecipeDef, ThingDef};
pub use crate::defs::{HediffDef, ResearchProjectDef, TerrainDef};
pub use crate::patch::{Operation, Patch, PatchSet};
pub use crate::xml::{Element, Node, Xml};
