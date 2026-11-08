//! A deliberately broad "kitchen sink" mod for stress-testing rim-rs (and
//! RimWorld 1.6 itself).
//!
//! Unlike `hello-mod` (a happy-path demo), this example intentionally touches
//! every feature area a real mod would use, borrowing structure from the
//! vanilla 1.6 Def XML shipped with the local RimWorld install:
//!
//! | Area | Defs | How it is built |
//! |---|---|---|
//! | Resource item | `RimRsWidget` | typed `ThingDef` |
//! | Melee weapon | `RimRsWeapon_Shiv` | typed + untyped extras (tools, stuff) |
//! | Apparel | `RimRsApparel_Cap` | typed + untyped extras (apparel node) |
//! | Implant item | `RimRsOrganItem` | typed + extras (costList, techHediffsTags) |
//! | Wall | `RimRsWall` | typed + extras (building node) |
//! | Workbench | `RimRsWorkbench` | typed (uses vanilla `BenchBase` parent) |
//! | Creature | `RimRsCreature` + PawnKindDef | typed + generic `Def` |
//! | Body/organ | `RimRsCreatureBody`, `RimRsOrganPartCrystal` | generic `Def` |
//! | Hediff/implant | `RimRsCrystalHeart` | typed `HediffDef` |
//! | Craft recipe | `Make_RimRsWidget`, `Make_RimRsOrganItem` | typed `RecipeDef` |
//! | Surgery recipe | `Install_RimRsCrystalHeart` | typed + extras (surgery) |
//! | Research | `RimRsCrystalTech[Advanced]` | typed + view-position extras |
//! | Terrain | `RimRsFloor` | typed `TerrainDef` |
//! | Custom stat | `RimRsCrystalResonance` | generic `Def` |
//! | Ideology precept | `RimRsCrystalCult*` | generic `Def` (DLC probe) |
//! | Patches | add/replace/remove | `PatchSet` (incl. an inherited-node probe) |
//! | C# | DefOf-style load harness | `csharp(...)` |
//!
//! Known deliberate gaps (missing-texture probes): `Graphic_Multi` entries
//! (workbench, creature) ship with a single texture; the remaining directions
//! render RimWorld's magenta fallback in-game on purpose.
//!
//! Run with:
//!
//! ```text
//! python3 scripts/make_textures.py   # once, after modifying textures list
//! cargo run -p kitchen-sink
//! ```

use std::path::PathBuf;

use rim_gen::prelude::*;

// ---------------------------------------------------------------- helpers

/// `<name>value</name>` shorthand for passing leaf children to typed
/// builders (they only expose the generic `child` escape hatch today).
fn txt(name: &str, value: &str) -> Element {
    Element::new(name).text(value)
}

/// Adds `<thingCategories><li>...` without touching the typed builder.
fn thing_categories(names: &[&str]) -> Element {
    let mut categories = Element::new("thingCategories");
    for name in names {
        categories = categories.child_text("li", *name);
    }
    categories
}

// ---------------------------------------------------------------- items

fn make_widget() -> ThingDef {
    ThingDef::new("RimRsWidget")
        .parent("ResourceBase")
        .label("rim-rs widget")
        .description("An ordinary manufactured widget produced as a test.")
        .texture("Things/Item/RimRsWidget")
        .stack_limit(75)
        .hit_points(100)
        .market_value(30)
        .mass(0.4)
        .child(thing_categories(&["Manufactured"]))
}

fn make_shiv() -> ThingDef {
    ThingDef::new("RimRsWeapon_Shiv")
        // Cross-file abstract dating back to Core: exercises XML inheritance
        // from an external mod file (BaseMeleeWeapon -> BaseWeapon chains).
        .parent("BaseMeleeWeapon_Sharp_Quality")
        .label("rim-rs shiv")
        .description("A test melee weapon made from a widget and hope.")
        .texture("Things/Item/Weapons/RimRsShiv")
        .stat_base("WorkToMake", 1800)
        .stat_base("Mass", 0.5)
        .market_value(80)
        // costList / stuffCategories / tools come from the abstract chain.
        .child(txt("costStuffCount", "30"))
        .child(thing_categories(&["WeaponsMelee"]))
        .child(txt("equippedAngleOffset", "-65"))
        .child(
            Element::new("tools")
                .child(
                    Element::new("li")
                        .child_text("label", "handle")
                        .child(Element::new("capacities").child_text("li", "Blunt"))
                        .child_text("power", "9")
                        .child_text("cooldownTime", "2"),
                )
                .child(
                    Element::new("li")
                        .child_text("label", "blade")
                        .child(Element::new("capacities").child_text("li", "Cut"))
                        .child_text("power", "13")
                        .child_text("cooldownTime", "1.5"),
                ),
        )
}

fn make_cap() -> ThingDef {
    ThingDef::new("RimRsApparel_Cap")
        .parent("ApparelMakeableBase")
        .label("rim-rs cap")
        .description("A cap for testing the apparel node and worn graphics.")
        .texture("Things/Pawn/Humanlike/RimRsCap/RimRsCap")
        .market_value(60)
        .stat_base("WorkToMake", 9000)
        .stat_base("Mass", 0.3)
        .stat_base("StuffEffectMultiplierArmor", 0.2)
        .stat_base("EquipDelay", 1.5)
        .child(txt("costStuffCount", "30"))
        .child(thing_categories(&["ApparelMisc"]))
        .child(
            Element::new("apparel")
                .child(Element::new("bodyPartGroups").child_text("li", "FullHead"))
                // Apparel layer defName is `Overhead` (lowercase `h`); `OverHead` does not resolve.
                .child(Element::new("layers").child_text("li", "Overhead"))
                .child(Element::new("tags").child_text("li", "Neolithic")),
        )
}

fn make_organ_item() -> ThingDef {
    ThingDef::new("RimRsOrganItem")
        .parent("BodyPartProstheticBase")
        .label("imbued crystal organ")
        .description("A crystal organ crafted at the workbench before surgery.")
        .texture("Things/Item/RimRsOrganItem")
        .child(
            Element::new("costList")
                .child_text("Steel", "20")
                .child_text("ComponentIndustrial", "2"),
        )
        .child(Element::new("techHediffsTags").child_text("li", "Simple"))
        // Vanilla-style craft route: recipeMaker ON the item def generates a
        // recipe named `Make_<defName>` automatically. Defining an explicit
        // RecipeDef with that same name collides ("Adding duplicate name") -
        // exactly what the first run showed, so the explicit Make_RimRsOrganItem
        // recipe was replaced by this mechanism. Vanilla never writes explicit
        // Make_* recipes for implant items.
        .child(
            Element::new("recipeMaker")
                .child_text("workSpeedStat", "GeneralLaborSpeed")
                .child_text("workSkill", "Crafting")
                .child_text("effectWorking", "Smith")
                .child_text("soundWorking", "Recipe_Machining")
                .child_text("unfinishedThingDef", "UnfinishedHealthItemProsthetic")
                .child_text("researchPrerequisite", "RimRsCrystalTechAdvanced")
                .child(Element::new("skillRequirements").child_text("Crafting", "6"))
                .child(Element::new("recipeUsers").child_text("li", "RimRsWorkbench"))
                .child_text("displayPriority", "600"),
        )
}

// ---------------------------------------------------------------- buildings

fn make_wall() -> ThingDef {
    ThingDef::new("RimRsWall")
        .parent("BuildingBase")
        .label("rim-rs wall")
        .description("A test wall. Holds a roof, blocks movement, lights and wind.")
        .texture("Things/Building/RimRsWall")
        .stack_limit(1)
        .stat_base("MaxHitPoints", 300)
        .stat_base("WorkToBuild", 135)
        .stat_base("Flammability", 0.1)
        .stat_base("MeditationFocusStrength", 0.1)
        .child(txt("altitudeLayer", "Building"))
        .child(txt("passability", "Impassable"))
        .child(txt("fillPercent", "1"))
        .child(txt("coversFloor", "true"))
        .child(txt("blockWind", "true"))
        .child(txt("blockLight", "true"))
        .child(txt("castEdgeShadows", "true"))
        .child(txt("staticSunShadowHeight", "1.0"))
        .child(txt("tickerType", "Never"))
        .child(txt("rotatable", "false"))
        .child(txt("selectable", "true"))
        .child(txt("neverMultiSelect", "true"))
        .child(txt("holdsRoof", "true"))
        .child(txt("drawStyleCategory", "Walls"))
        .child(txt("designationCategory", "Structure"))
        .child(txt("uiOrder", "2000"))
        .child(txt("fertility", "0"))
        .child(txt("terrainAffordanceNeeded", "Medium"))
        .child(txt("canOverlapZones", "false"))
        .child(txt("costStuffCount", "5"))
        .child(
            Element::new("stuffCategories")
                .child_text("li", "Metallic")
                .child_text("li", "Woody")
                .child_text("li", "Stony"),
        )
        .child(
            Element::new("building")
                .child_text("isInert", "true")
                .child_text("isWall", "true")
                .child_text("supportsWallAttachments", "true"),
        )
}

fn make_workbench() -> ThingDef {
    ThingDef::new("RimRsWorkbench")
        .parent("BenchBase")
        .label("rim-rs workbench")
        .description("A test workbench. Uses Graphic_Multi but only ships one texture: the other three directions render the vanilla missing-texture checker, on purpose.")
        .thing_class("Building_WorkTable")
        // Single texture on purpose - see the module docs.
        .texture("Things/Building/RimRsWorkbench")
        .graphic_class("Graphic_Multi")
        .draw_size(2.6, 1.3)
        .size(2.0, 1.0)
        .stat_base("WorkToBuild", 1200)
        .stat_base("MaxHitPoints", 120)
        .stat_base("Flammability", 1.0)
        .stat_base("Beauty", -1)
        .child(Element::new("costList").child_text("Steel", "50"))
        .child(txt("altitudeLayer", "Building"))
        .child(txt("fillPercent", "0.5"))
        .child(txt("passability", "PassThroughOnly"))
        .child(txt("pathCost", "60"))
        .child(txt("designationCategory", "Production"))
        .child(txt("uiOrder", "9001"))
        .child(txt("hasInteractionCell", "True"))
        .child(txt("interactionCellOffset", "(0,0,-1)"))
        .child(txt("surfaceType", "Item"))
        .child(Element::new("inspectorTabs").child_text("li", "ITab_Bills"))
        // Locks one of the research defs - tests the research -> def chain.
        .child(Element::new("researchPrerequisites").child_text("li", "RimRsCrystalTech"))
}

// ---------------------------------------------------------------- creature + body + organ

fn make_creature() -> ThingDef {
    ThingDef::new("RimRsCreature")
        .parent("AnimalThingBase")
        .label("rim-rs creature")
        .description("A lazy quadruped that exists purely so organ surgery can be tested on it.")
        .stat_base("MoveSpeed", 4.2)
        .stat_base("MarketValue", 250)
        .stat_base("Wildness", 0.8)
        .stat_base("ComfyTemperatureMin", -20)
        .stat_base("RimRsCrystalResonance", 1.0)
        // Our own body def (RimRsCreatureBody) containing our own organ part.
        .child(
            Element::new("race")
                .child_text("body", "RimRsCreatureBody")
                .child_text("meatLabel", "rim-rs creature meat")
                .child_text("baseBodySize", "1")
                .child_text("baseHealthScale", "1")
                .child_text("baseHungerRate", "0.25")
                .child_text("foodType", "OmnivoreAnimal")
                .child_text("trainability", "Intermediate")
                .child_text("nameOnTameChance", "0.5")
                .child_text("lifeExpectancy", "15")
                .child_text("gestationPeriodDays", "12")
                .child(
                    Element::new("lifeStageAges")
                        .child(
                            Element::new("li")
                                .child_text("def", "AnimalBaby")
                                .child_text("minAge", "0"),
                        )
                        .child(
                            Element::new("li")
                                .child_text("def", "AnimalJuvenile")
                                .child_text("minAge", "0.2"),
                        )
                        .child(
                            Element::new("li")
                                .child_text("def", "AnimalAdult")
                                .child_text("minAge", "0.5"),
                        ),
                ),
        )
        // Tools reference groups defined inside RimRsCreatureBody.
        .child(
            Element::new("tools")
                .child(
                    Element::new("li")
                        .child_text("label", "left claw")
                        .child_text("labelNoLocation", "claw")
                        .child(Element::new("capacities").child_text("li", "Scratch"))
                        .child_text("power", "9")
                        .child_text("cooldownTime", "2")
                        .child_text("linkedBodyPartsGroup", "FrontLeftPaw"),
                )
                .child(
                    Element::new("li")
                        .child_text("label", "right claw")
                        .child_text("labelNoLocation", "claw")
                        .child(Element::new("capacities").child_text("li", "Scratch"))
                        .child_text("power", "9")
                        .child_text("cooldownTime", "2")
                        .child_text("linkedBodyPartsGroup", "FrontRightPaw"),
                )
                .child(
                    Element::new("li")
                        .child_text("label", "head")
                        .child_text("labelNoLocation", "head")
                        .child(Element::new("capacities").child_text("li", "Blunt"))
                        .child_text("power", "6")
                        .child_text("cooldownTime", "2")
                        .child_text("linkedBodyPartsGroup", "HeadAttackTool")
                        .child_text("ensureLinkedBodyPartsGroupAlwaysUsable", "true"),
                ),
        )
}

fn make_pawn_kind() -> Def {
    Def::new("PawnKindDef")
        .child_text("defName", "RimRsCreature")
        .child_text("label", "rim-rs creature")
        .child_text("labelPlural", "rim-rs creatures")
        .child_text("race", "RimRsCreature")
        .child_text("combatPower", "55")
        // 1.6 note: the old `wildSpawn_spawnWildness` field no longer exists.
        // Wild fauna spawning is driven by `ecoSystemWeight` (plus a
        // `wildGroupSize` range); the probe is whether our kind shows up.
        .child_text("ecoSystemWeight", "0.6")
        .child_text("wildGroupSize", "1~3")
        .child(
            Element::new("lifeStages")
                .child(
                    Element::new("li").child(
                        Element::new("bodyGraphicData")
                            .child_text("texPath", "Pawn/Animal/RimRsCreature/RimRsCreature")
                            .child_text("drawSize", "0.9"),
                    ),
                )
                .child(
                    Element::new("li").child(
                        Element::new("bodyGraphicData")
                            .child_text("texPath", "Pawn/Animal/RimRsCreature/RimRsCreature")
                            .child_text("drawSize", "1.2"),
                    ),
                )
                .child(
                    Element::new("li").child(
                        Element::new("bodyGraphicData")
                            .child_text("texPath", "Pawn/Animal/RimRsCreature/RimRsCreature")
                            .child_text("drawSize", "1.5"),
                    ),
                ),
        )
}

fn make_organ_part() -> Def {
    Def::new("BodyPartDef")
        .child_text("defName", "RimRsOrganPartCrystal")
        .child_text("label", "imbued crystal")
        .child_text("hitPoints", "14")
        .child_text("bleedRate", "2")
        // Drops our organ item when removed - links BodyPartDef <-> ThingDef.
        .child_text("spawnThingOnRemoved", "RimRsOrganItem")
        .child_text("canSuggestAmputation", "false")
}

fn make_body() -> Def {
    // Coverage values are relative to the parent part; simplified copy of the
    // vanilla quadruped body. Our organ sits in the torso next to the heart.
    let part = |def: &str, coverage: &str, extra: Element| {
        Element::new("li")
            .child_text("def", def)
            .child_text("coverage", coverage)
            .child(extra)
    };

    let leg = |label: &str, group: Option<&str>| {
        let mut paw = Element::new("li")
            .child_text("def", "Paw")
            .child_text("customLabel", format!("{} paw", label))
            .child_text("coverage", "0.15");
        if let Some(name) = group {
            paw = paw.child(Element::new("groups").child_text("li", name));
        }
        Element::new("li")
            .child_text("def", "Leg")
            .child_text("customLabel", format!("{} leg", label))
            .child_text("coverage", "0.07")
            .child_text("height", "Bottom")
            // Child records must be wrapped in <parts> (the game error only
            // surfaces the unwrapped <li>, which makes this easy to miss).
            .child(Element::new("parts").child(paw))
    };

    let head = Element::new("li")
        .child_text("def", "Head")
        .child_text("coverage", "0.75")
        .child(Element::new("groups").child_text("li", "HeadAttackTool"))
        .child(
            Element::new("parts")
                .child(
                    Element::new("li")
                        .child_text("def", "Skull")
                        .child_text("coverage", "0.25")
                        .child_text("depth", "Inside")
                        .child(
                            Element::new("parts").child(
                                Element::new("li")
                                    .child_text("def", "Brain")
                                    .child_text("coverage", "0.70")
                                    .child_text("depth", "Inside"),
                            ),
                        ),
                )
                .child(part("Eye", "0.12", txt("customLabel", "left eye")))
                .child(part("Eye", "0.12", txt("customLabel", "right eye")))
                .child(part("Ear", "0.08", txt("customLabel", "left ear")))
                .child(part("Ear", "0.08", txt("customLabel", "right ear")))
                .child(part("Nose", "0.10", txt("customLabel", "nose")))
                .child(
                    Element::new("li")
                        .child_text("def", "AnimalJaw")
                        .child_text("coverage", "0.10")
                        .child(Element::new("groups").child_text("li", "Teeth")),
                ),
        );

    Def::new("BodyDef")
        .child_text("defName", "RimRsCreatureBody")
        .child_text("label", "rim-rs creature body")
        .child(
            Element::new("corePart")
                .child_text("def", "Body")
                .child_text("height", "Middle")
                .child_text("depth", "Outside")
                .child(
                    Element::new("parts")
                        .child(part("Spine", "0.03", txt("depth", "Inside")))
                        .child(part("Stomach", "0.03", txt("depth", "Inside")))
                        .child(part("Heart", "0.03", txt("depth", "Inside")))
                        .child(part(
                            "RimRsOrganPartCrystal",
                            "0.05",
                            txt("customLabel", "imbued crystal"),
                        ))
                        .child(part("Lung", "0.03", txt("customLabel", "left lung")))
                        .child(part("Lung", "0.03", txt("customLabel", "right lung")))
                        .child(part("Kidney", "0.03", txt("customLabel", "left kidney")))
                        .child(part("Kidney", "0.03", txt("customLabel", "right kidney")))
                        .child(part("Liver", "0.03", txt("depth", "Inside")))
                        .child(
                            Element::new("li")
                                .child_text("def", "Neck")
                                .child_text("coverage", "0.22")
                                .child_text("height", "Top")
                                .child(Element::new("parts").child(head)),
                        )
                        .child(leg("front left", Some("FrontLeftPaw")))
                        .child(leg("front right", Some("FrontRightPaw")))
                        .child(leg("rear left", None))
                        .child(leg("rear right", None)),
                ),
        )
}

// ---------------------------------------------------------------- hediff + recipes

fn make_crystal_heart_hediff() -> HediffDef {
    HediffDef::new("RimRsCrystalHeart")
        // Abstract from Core - a cross-mod parent reference for XML inheritance.
        .parent("ImplantHediffBase")
        .label("imbued crystal organ")
        .description("An installed imbued crystal organ. Slightly better than the original.")
        .child(
            Element::new("addedPartProps")
                .child_text("solid", "true")
                .child_text("partEfficiency", "0.85"),
        )
        .child(txt("spawnThingOnRemoved", "RimRsOrganItem"))
        .child(
            Element::new("descriptionHyperlinks")
                .child(Element::new("ThingDef").text("RimRsOrganItem")),
        )
        // HediffStage is not exported by the crate today (finding: the typed
        // stage builder cannot be reached from outside), so write <stages>
        // through the child escape hatch instead. Keep the anthropomorphic
        // single-stage shape identical to what a typed stage would have
        // emitted, including a statOffset on the custom StatDef.
        .child(
            Element::new("stages").child(
                Element::new("li")
                    .child_text("label", "imbued")
                    .child_text("becomeVisible", "True")
                    .child(Element::new("statOffsets").child_text("RimRsCrystalResonance", "0.2")),
            ),
        )
}

fn make_widget_recipe() -> RecipeDef {
    RecipeDef::new("Make_RimRsWidget")
        .label("make a rim-rs widget")
        .description("Shape wood into a widget at any crafting surface.")
        .job_string("Making a rim-rs widget.")
        .work_amount(500.0)
        .work_speed_stat("GeneralLaborSpeed")
        .work_skill("Crafting")
        // Wood resource defName is `WoodLog` in 1.6 — the old `Wood_Log` name
        // no longer resolves (first-run finding).
        .ingredient("WoodLog", 30)
        .product("RimRsWidget", 1)
        .recipe_user("CraftingSpot")
}

fn make_surgery_recipe() -> RecipeDef {
    // The vanilla surgery abstract carries workerClass=Recipe_InstallImplant,
    // workAmount, skillRequirements and recipeUsers (Human + CreepJoiner).
    // Our ingredients node is complete on this def and per XML-inheritance
    // rules list entries merge onto the parent's: the surgery may demand the
    // organ item *and* the inherited 2 medicine? Whichever way RimWorld
    // resolves it, the in-game log + bill UI will show the result. That
    // merge behaviour is exactly what we want to observe here.
    RecipeDef::new("Install_RimRsCrystalHeart")
        .parent("SurgeryInstallImplantBase")
        .label("install imbued crystal organ")
        .description("Install the imbued crystal organ - a test surgery.")
        .job_string("Installing imbued crystal organ.")
        .ingredient("RimRsOrganItem", 1)
        .fixed_ingredient_filter(|f| f.allow_thing(["RimRsOrganItem"]))
        .child(Element::new("appliedOnFixedBodyParts").child_text("li", "RimRsOrganPartCrystal"))
        .child(txt("addsHediff", "RimRsCrystalHeart"))
        .child(txt("deathOnFailedSurgeryChance", "0.2"))
        .recipe_user("RimRsCreature")
}

// ---------------------------------------------------------------- research / terrain / stat

fn research_defs() -> Vec<ResearchProjectDef> {
    vec![
        ResearchProjectDef::new("RimRsCrystalTech")
            .label("rim-rs crystallography")
            .description("Unlock the rim-rs workbench.")
            .base_cost(600)
            .tech_level("Industrial")
            .child(txt("researchViewX", "2"))
            .child(txt("researchViewY", "3")),
        ResearchProjectDef::new("RimRsCrystalTechAdvanced")
            .label("advanced rim-rs surgery")
            .description("Unlock crafting the imbued crystal organ.")
            .base_cost(1200)
            .tech_level("Industrial")
            .prerequisite("RimRsCrystalTech")
            .child(txt("researchViewX", "4"))
            .child(txt("researchViewY", "3")),
    ]
}

fn make_floor() -> TerrainDef {
    TerrainDef::new("RimRsFloor")
        .parent("FloorBase")
        .label("rim-rs floor")
        .description("A test floor tile, edge type Hard, tinted checker texture.")
        .texture_path("Things/Floor/RimRsFloor")
        .edge_type("Hard")
        .child(txt("renderPrecedence", "262"))
        .stat_base("WorkToBuild", 550)
        .stat_base("Beauty", 0)
        .stat_base("CleaningTimeFactor", 0.27)
}

fn make_stat() -> Def {
    Def::new("StatDef")
        .child_text("defName", "RimRsCrystalResonance")
        .child_text("label", "crystal resonance")
        .child_text(
            "description",
            "A custom test stat. Referenced by the creature's statBases and the implant hediff.",
        )
        .child_text("category", "Basics")
        .child_text("defaultBaseValue", "1")
        .child_text("minValue", "0")
        .child_text("toStringStyle", "FloatTwo")
}

// ---------------------------------------------------------------- ideology precepts (DLC probe)

fn precept_defs() -> Vec<Def> {
    // Minimal custom ideology content: an issue plus a precept. There are no
    // thought comps below the precept - probing whether a comp-less precept
    // loads. If Ideology is active the custom issue will simply not show up
    // in generated ideos; loading is what we verify here.
    vec![
        Def::new("IssueDef")
            .child_text("defName", "RimRsCrystalCult")
            .child_text("label", "crystal worship")
            .child_text("iconPath", "UI/Issues/AnimalSlaughter"), // reference a vanilla icon on purpose
        Def::new("PreceptDef")
            .child_text("defName", "RimRsCrystalCult_Mandatory")
            .child_text("issue", "RimRsCrystalCult")
            .child_text("label", "crystal worship mandatory")
            .child_text(
                "description",
                "All colonists must worship the imbued crystals.",
            )
            .child_text("impact", "Medium")
            .child_text("displayOrderInIssue", "10")
            .child_text("displayOrderInImpact", "50"),
    ]
}

// ---------------------------------------------------------------- C# load harness

const CS_HARNESS: &str = r#"using System.Collections.Generic;
using System.Linq;
using RimWorld;
using Verse;

[StaticConstructorOnStartup]
public static class RimRsKitchenSinkMod
{
    static RimRsKitchenSinkMod()
    {
        int missing = 0;
        var lines = new List<string>();

        void Check<TDef>(string defName) where TDef : Def
        {
            var def = DefDatabase<TDef>.GetNamedSilentFail(defName);
            if (def == null)
            {
                missing++;
                lines.Add("MISSING: " + defName + " (" + typeof(TDef).Name + ")");
            }
        }

        Check<ThingDef>("RimRsWidget");
        Check<ThingDef>("RimRsWeapon_Shiv");
        Check<ThingDef>("RimRsApparel_Cap");
        Check<ThingDef>("RimRsOrganItem");
        Check<ThingDef>("RimRsWall");
        Check<ThingDef>("RimRsWorkbench");
        Check<ThingDef>("RimRsCreature");
        Check<RecipeDef>("Make_RimRsWidget");
        Check<RecipeDef>("Install_RimRsCrystalHeart");
        Check<BodyDef>("RimRsCreatureBody");
        Check<BodyPartDef>("RimRsOrganPartCrystal");
        Check<PawnKindDef>("RimRsCreature");
        Check<HediffDef>("RimRsCrystalHeart");
        Check<ResearchProjectDef>("RimRsCrystalTech");
        Check<ResearchProjectDef>("RimRsCrystalTechAdvanced");
        Check<TerrainDef>("RimRsFloor");
        Check<StatDef>("RimRsCrystalResonance");
        Check<PreceptDef>("RimRsCrystalCult_Mandatory");
        Check<IssueDef>("RimRsCrystalCult");

        // Cross-links
        ThingDef widget = DefDatabase<ThingDef>.GetNamedSilentFail("RimRsWidget");
        if (widget != null)
            Log.Message("[rim-rs kitchen-sink] patch replace probe: widget MarketValue = "
                        + widget.GetStatValueAbstract(StatDefOf.MarketValue) + " (99 means the Replace patch won)");

        ThingDef machining = DefDatabase<ThingDef>.GetNamedSilentFail("TableMachining");
        RecipeDef makeWidget = DefDatabase<RecipeDef>.GetNamedSilentFail("Make_RimRsWidget");
        if (machining != null && makeWidget != null)
            Log.Message("[rim-rs kitchen-sink] patch probe: TableMachining knows Make_RimRsWidget = " + machining.AllRecipes.Contains(makeWidget));

        ThingDef bench = DefDatabase<ThingDef>.GetNamedSilentFail("RimRsWorkbench");
        if (bench != null)
        {
            // The craft recipe for the organ is auto-generated from the item's
            // recipeMaker (vanilla-style). Verify it ends up on our bench.
            // (products is a List<ThingDefCountClass>, not a dictionary.)
            bool organCraftOnBench = false;
            foreach (RecipeDef r in bench.AllRecipes)
                if (r.products != null && r.products.Any(p => p.thingDef.defName == "RimRsOrganItem")) organCraftOnBench = true;
            RecipeDef auto = DefDatabase<RecipeDef>.GetNamedSilentFail("Make_RimRsOrganItem");
            Log.Message("[rim-rs kitchen-sink] bench probe: organ craft on workbench = " + organCraftOnBench
                        + ", auto recipe 'Make_RimRsOrganItem' registered = " + (auto != null));
        }

        RecipeDef surgery = DefDatabase<RecipeDef>.GetNamedSilentFail("Install_RimRsCrystalHeart");
        ThingDef creature = DefDatabase<ThingDef>.GetNamedSilentFail("RimRsCreature");
        if (creature != null)
        {
            Log.Message("[rim-rs kitchen-sink] creature probe: surgery recipes on creature = " + creature.AllRecipes.Count
                        + " (contains install surgery = " + (surgery != null && creature.AllRecipes.Contains(surgery)) + ")");
            PawnKindDef kind = DefDatabase<PawnKindDef>.GetNamedSilentFail("RimRsCreature");
            Log.Message("[rim-rs kitchen-sink] creature probe: body = " + (creature.race?.body?.defName ?? "none")
                        + ", pawnkind race link = " + (kind?.race?.defName ?? "none"));
        }

        if (surgery != null)
            Log.Message("[rim-rs kitchen-sink] surgery probe: ingredients = " + surgery.ingredients.Count
                        + ", addsHediff = " + surgery.addsHediff?.defName
                        + ", targetPart = " + (surgery.appliedOnFixedBodyParts != null ? string.Join(",", surgery.appliedOnFixedBodyParts) : "none"));

        StatDef resonance = DefDatabase<StatDef>.GetNamedSilentFail("RimRsCrystalResonance");
        if (creature != null && resonance != null)
            Log.Message("[rim-rs kitchen-sink] stat probe: creature " + resonance.defName + " = "
                        + creature.GetStatValueAbstract(resonance).ToString("0.00"));

        Log.Message(missing == 0
            ? "[rim-rs kitchen-sink] ALL 19 EXPLICIT DEFS LOADED OK (plus 1 auto recipe)"
            : "[rim-rs kitchen-sink] " + missing + " DEF(S) MISSING:\n" + string.Join("\n", lines));
    }
}
"#;

// ---------------------------------------------------------------- main

fn main() -> Result<()> {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));

    let research = research_defs();
    let precepts = precept_defs();

    let report = Mod::builder("Rim RS Kitchen Sink")
        .package_id("miquella.kitchensink")
        .author("miquella")
        .description(
            "Kitchen-sink stress test for rim-rs: item, weapon, apparel, wall, \
             workbench, recipes, surgery, body/organ, creature, research, \
             terrain, custom stat, ideology precept, patches and a C# load harness.",
        )
        .url("https://example.invalid/rim-rs")
        .supported_versions(["1.6"])
        .defs(|defs| {
            defs.file("RimRsItems", |f| {
                f.add(make_widget());
                f.add(make_shiv());
                f.add(make_cap());
                f.add(make_organ_item());
            });

            defs.file("RimRsBuildings", |f| {
                f.add(make_wall());
                f.add(make_workbench());
            });

            defs.file("RimRsCreature", |f| {
                f.add(make_body());
                f.add(make_organ_part());
                f.add(make_creature());
                f.add(make_pawn_kind());
            });

            defs.file("RimRsHediffs", |f| {
                f.add(make_crystal_heart_hediff());
            });

            defs.file("RimRsRecipes", |f| {
                f.add(make_widget_recipe());
                f.add(make_surgery_recipe());
            });

            defs.file("RimRsResearch", |f| {
                for project in research {
                    f.add(project);
                }
            });

            defs.file("RimRsTerrain", |f| {
                f.add(make_floor());
            });

            defs.file("RimRsMisc", |f| {
                f.add(make_stat());
            });

            defs.file("RimRsPrecepts", |f| {
                for precept in precepts {
                    f.add(precept);
                }
            });
        })
        .patches(|patches| {
            // 1. Add: hook the widget recipe onto a vanilla machining table too.
            patches.add(Patch::new("RimRsTweakAdd").op("PatchOperationAdd", |op| {
                op.xpath(r#"Defs/ThingDef[defName="TableMachining"]/recipes"#)
                    .raw_value("<li>Make_RimRsWidget</li>");
            }));
            // 2. Replace: rewrite our own def's MarketValue.
            patches.add(
                Patch::new("RimRsTweakReplace").op("PatchOperationReplace", |op| {
                    op.xpath(r#"Defs/ThingDef[defName="RimRsWidget"]/statBases/MarketValue"#)
                        .raw_value("<MarketValue>99</MarketValue>");
                }),
            );
            // 3. Remove on an INHERITED node (burnableByRecipe comes from the
            //    abstract ApparelNoQualityBase): probes whether patch operations
            //    run before or after XML inheritance resolution. Either way is
            //    a finding: the game log (or the successful removal) decides.
            patches.add(
                Patch::new("RimRsTweakRemove").op("PatchOperationRemove", |op| {
                    op.xpath(r#"Defs/ThingDef[defName="RimRsApparel_Cap"]/burnableByRecipe"#);
                }),
            );
        })
        .csharp("RimRsKitchenSink.cs", CS_HARNESS)
        .assets(|assets| {
            assets.textures(root.join("assets/textures"));
            assets.preview(root.join("assets/preview.png"));
        })
        .options(
            // deploy(true): copy the staged mod straight into the game's Mods/
            // directory (auto-probe finds ~/Games/RimWorld). The overwrite
            // guard keeps this safe: it refuses to replace another mod's dir.
            BuildOptions::default().base_dir(&root).deploy(true),
        )
        .build()?;

    println!("mod dir:   {:?}", report.mod_dir);
    println!("deployed:  {:?}", report.deployed_dir);
    println!("dll:       {:?}", report.dll_path);
    println!("zip:       {:?}", report.zip_path);
    println!();
    println!("Next steps:");
    println!("  1. Launch RimWorld 1.6 and enable 'Rim RS Kitchen Sink'.");
    println!("  2. Watch the log for [rim-rs kitchen-sink] probes and red errors.");
    println!("  3. In dev mode: spawn the creature, view the tech tree, build the");
    println!("     wall/workbench/floor, craft + install the crystal organ.");

    Ok(())
}
