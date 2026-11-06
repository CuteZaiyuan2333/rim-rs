use crate::defs::Def;
use crate::xml::{Element, Node};

/// Typed builder for a `TerrainDef` (floors, paths and other terrain layers).
///
/// Only common fields are typed. Use [`TerrainDef::child`] /
/// [`TerrainDef::raw`] for anything else, e.g. `constructEffect` or
/// `designatorDropdown`.
#[derive(Debug, Clone)]
pub struct TerrainDef {
    def_name: String,
    parent: Option<String>,
    label: Option<String>,
    description: Option<String>,
    texture_path: Option<String>,
    layer: Option<String>,
    edge_type: Option<String>,
    fertility: Option<f32>,
    walk_speed: Option<f32>,
    path_cost: Option<u32>,
    affordances: Vec<String>,
    stat_bases: Vec<(String, String)>,
    research_prerequisites: Vec<String>,
    extras: Vec<Node>,
}

impl TerrainDef {
    pub fn new(def_name: impl Into<String>) -> Self {
        Self {
            def_name: def_name.into(),
            parent: None,
            label: None,
            description: None,
            texture_path: None,
            layer: None,
            edge_type: None,
            fertility: None,
            walk_speed: None,
            path_cost: None,
            affordances: Vec::new(),
            stat_bases: Vec::new(),
            research_prerequisites: Vec::new(),
            extras: Vec::new(),
        }
    }

    pub fn def_name(&self) -> &str {
        &self.def_name
    }

    /// Sets the `ParentName` attribute.
    pub fn parent(mut self, parent: impl Into<String>) -> Self {
        self.parent = Some(parent.into());
        self
    }

    pub fn label(mut self, label: impl Into<String>) -> Self {
        self.label = Some(label.into());
        self
    }

    pub fn description(mut self, description: impl Into<String>) -> Self {
        self.description = Some(description.into());
        self
    }

    /// Sets `texturePath` (terrain textures are declared directly with a path,
    /// not through `graphicData`).
    pub fn texture_path(mut self, path: impl Into<String>) -> Self {
        self.texture_path = Some(path.into());
        self
    }

    /// Sets `layer`, e.g. `Floor`. Omit for natural terrain.
    pub fn layer(mut self, layer: impl Into<String>) -> Self {
        self.layer = Some(layer.into());
        self
    }

    /// Sets `edgeType`, e.g. `Hard` or `Fuzzy`.
    pub fn edge_type(mut self, edge_type: impl Into<String>) -> Self {
        self.edge_type = Some(edge_type.into());
        self
    }

    /// Sets `fertility` (crops grow when above 0).
    pub fn fertility(mut self, fertility: f32) -> Self {
        self.fertility = Some(fertility);
        self
    }

    /// Sets `walkSpeed` (relative; 1 is normal walking).
    pub fn walk_speed(mut self, speed: f32) -> Self {
        self.walk_speed = Some(speed);
        self
    }

    /// Sets `pathCost` (0-150; >150 is impassable).
    pub fn path_cost(mut self, cost: u32) -> Self {
        self.path_cost = Some(cost);
        self
    }

    /// Adds an affordance (`<affordances><li>...`), e.g. `Light`, `Smooth`,
    /// `Clean`.
    pub fn affordance(mut self, affordance: impl Into<String>) -> Self {
        self.affordances.push(affordance.into());
        self
    }

    /// Adds a stat to `<statBases>`, e.g. `("Cleanliness", 0.5)`.
    pub fn stat_base(mut self, name: impl Into<String>, value: impl ToString) -> Self {
        self.stat_bases.push((name.into(), value.to_string()));
        self
    }

    /// Adds a `researchPrerequisites` entry (`<li>...`).
    pub fn research_prerequisite(mut self, def_name: impl Into<String>) -> Self {
        self.research_prerequisites.push(def_name.into());
        self
    }

    /// Adds an arbitrary child element/raw node.
    pub fn child(mut self, child: impl Into<Node>) -> Self {
        self.extras.push(child.into());
        self
    }

    /// Adds a raw XML fragment.
    pub fn raw(mut self, raw: impl Into<String>) -> Self {
        self.extras.push(Node::Raw(raw.into()));
        self
    }

    /// Builds the underlying [`Def`].
    pub fn build(self) -> Def {
        let mut root = Element::new("TerrainDef");
        if let Some(parent) = &self.parent {
            root = root.attr("ParentName", parent);
        }

        root = root.child_text("defName", &self.def_name);

        if let Some(label) = &self.label {
            root = root.child_text("label", label);
        }
        if let Some(description) = &self.description {
            root = root.child_text("description", description);
        }
        if let Some(texture) = &self.texture_path {
            root = root.child_text("texturePath", texture);
        }
        if let Some(layer) = &self.layer {
            root = root.child_text("layer", layer);
        }
        if let Some(edge_type) = &self.edge_type {
            root = root.child_text("edgeType", edge_type);
        }
        if let Some(fertility) = self.fertility {
            root = root.child_text("fertility", fmt_num(fertility));
        }
        if let Some(speed) = self.walk_speed {
            root = root.child_text("walkSpeed", fmt_num(speed));
        }
        if let Some(cost) = self.path_cost {
            root = root.child_text("pathCost", cost.to_string());
        }

        if !self.affordances.is_empty() {
            let mut affordances = Element::new("affordances");
            for affordance in &self.affordances {
                affordances = affordances.child_text("li", affordance);
            }
            root = root.child(affordances);
        }

        if !self.stat_bases.is_empty() {
            let mut stat_bases = Element::new("statBases");
            for (name, value) in &self.stat_bases {
                stat_bases = stat_bases.child_text(name, value);
            }
            root = root.child(stat_bases);
        }

        if !self.research_prerequisites.is_empty() {
            let mut research = Element::new("researchPrerequisites");
            for prerequisite in &self.research_prerequisites {
                research = research.child_text("li", prerequisite);
            }
            root = root.child(research);
        }

        for extra in self.extras {
            root = root.child(extra);
        }

        Def::from(root)
    }
}

fn fmt_num(value: f32) -> String {
    if value.fract() == 0.0 {
        format!("{}", value as i64)
    } else {
        format!("{value}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::defs::DefSet;
    use crate::error::Error;

    #[test]
    fn builds_a_terrain_def() {
        let def = TerrainDef::new("RimRsCarpetPurple")
            .parent("FloorBase")
            .label("rim-rs purple carpet")
            .description("A <b>soft</b> purple carpet.")
            .texture_path("Terrain/RimRsCarpetPurple")
            .layer("Floor")
            .edge_type("Hard")
            .fertility(0.0)
            .walk_speed(1.1)
            .path_cost(1)
            .affordance("Smooth")
            .affordance("Clean")
            .stat_base("Cleanliness", 0.5)
            .stat_base("Beauty", 1.0)
            .research_prerequisite("ColoredLights")
            .build();

        let xml = def.into_element().to_xml_string().unwrap();
        assert!(xml.contains(r#"<TerrainDef ParentName="FloorBase">"#));
        assert!(xml.contains("<defName>RimRsCarpetPurple</defName>"));
        assert!(xml.contains("<texturePath>Terrain/RimRsCarpetPurple</texturePath>"));
        assert!(xml.contains("<layer>Floor</layer>"));
        assert!(xml.contains("<edgeType>Hard</edgeType>"));
        assert!(xml.contains("<fertility>0</fertility>"));
        assert!(xml.contains("<walkSpeed>1.1</walkSpeed>"));
        assert!(xml.contains("<pathCost>1</pathCost>"));
        assert!(xml.contains("<affordances>"));
        assert!(xml.contains("<li>Smooth</li>"));
        assert!(xml.contains("<statBases>"));
        assert!(xml.contains("<Cleanliness>0.5</Cleanliness>"));
        assert!(xml.contains("<researchPrerequisites>"));
        assert!(xml.contains("<li>ColoredLights</li>"));
    }

    #[test]
    fn rejects_an_invalid_def_name() {
        let mut set = DefSet::new();
        set.add(TerrainDef::new("bad/name").label("broken"));
        assert!(matches!(
            set.xml_files(),
            Err(Error::InvalidDefName(name)) if name == "bad/name"
        ));
    }
}
