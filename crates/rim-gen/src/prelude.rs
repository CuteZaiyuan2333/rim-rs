//! Convenient imports for building a mod.
//!
//! ```no_run
//! use rim_gen::prelude::*;
//!
//! # fn main() -> rim_gen::Result<()> {
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
pub use crate::builder::{BuildOptions, BuildReport, ModBuilder, ModBuilder as Mod, ModDependency};
pub use crate::defs::{Def, DefSet, IntoDef, RecipeDef, ThingDef, ThingFilter};
pub use crate::defs::{HediffDef, ResearchProjectDef, StuffCategoryDef, TerrainDef};
pub use crate::harmony::MaterialCarry;
pub use crate::patch::{Operation, Patch, PatchSet};
pub use crate::xml::{Element, Node, Xml};
