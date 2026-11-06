use crate::defs::Def;
use crate::xml::{Element, Node};

/// Typed builder for a `ResearchProjectDef`.
///
/// Only common fields are typed. Use [`ResearchProjectDef::child`] /
/// [`ResearchProjectDef::raw`] for anything else, e.g. `page` or `techLevel`.
#[derive(Debug, Clone)]
pub struct ResearchProjectDef {
    def_name: String,
    parent: Option<String>,
    label: Option<String>,
    description: Option<String>,
    base_cost: Option<u32>,
    tech_level: Option<String>,
    prerequisites: Vec<String>,
    extras: Vec<Node>,
}

impl ResearchProjectDef {
    pub fn new(def_name: impl Into<String>) -> Self {
        Self {
            def_name: def_name.into(),
            parent: None,
            label: None,
            description: None,
            base_cost: None,
            tech_level: None,
            prerequisites: Vec::new(),
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

    /// Sets the research work cost, e.g. `1000`.
    pub fn base_cost(mut self, cost: u32) -> Self {
        self.base_cost = Some(cost);
        self
    }

    /// Sets `techLevel`, e.g. `Industrial`.
    pub fn tech_level(mut self, level: impl Into<String>) -> Self {
        self.tech_level = Some(level.into());
        self
    }

    /// Adds a prerequisite research project (`<prerequisites><li>...`).
    pub fn prerequisite(mut self, def_name: impl Into<String>) -> Self {
        self.prerequisites.push(def_name.into());
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
        let mut root = Element::new("ResearchProjectDef");
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
        if let Some(cost) = self.base_cost {
            root = root.child_text("baseCost", cost.to_string());
        }
        if let Some(level) = &self.tech_level {
            root = root.child_text("techLevel", level);
        }

        if !self.prerequisites.is_empty() {
            let mut prerequisites = Element::new("prerequisites");
            for prerequisite in &self.prerequisites {
                prerequisites = prerequisites.child_text("li", prerequisite);
            }
            root = root.child(prerequisites);
        }

        for extra in self.extras {
            root = root.child(extra);
        }

        Def::from(root)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::defs::DefSet;
    use crate::error::Error;

    #[test]
    fn builds_a_research_project_def() {
        let def = ResearchProjectDef::new("RimRsCrafting")
            .label("rim-rs crafting")
            .description("Unlocks <b>rim-rs</b> recipes.")
            .base_cost(1000)
            .tech_level("Industrial")
            .prerequisite("BasicRefining")
            .build();

        let xml = def.into_element().to_xml_string().unwrap();
        assert!(xml.contains("<ResearchProjectDef>"));
        assert!(xml.contains("<defName>RimRsCrafting</defName>"));
        assert!(xml.contains("<baseCost>1000</baseCost>"));
        assert!(xml.contains("<techLevel>Industrial</techLevel>"));
        assert!(xml.contains("<prerequisites>"));
        assert!(xml.contains("<li>BasicRefining</li>"));
    }

    #[test]
    fn rejects_an_invalid_def_name() {
        let mut set = DefSet::new();
        set.add(ResearchProjectDef::new("bad\tname").label("broken"));
        assert!(matches!(
            set.xml_files(),
            Err(Error::InvalidDefName(name)) if name == "bad\tname"
        ));
    }
}
