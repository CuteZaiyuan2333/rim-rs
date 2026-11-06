use crate::defs::Def;
use crate::xml::{Element, Node};

/// Typed builder for a single HediffDef stage.
///
/// Stages are rendered as `<stages><li>...</li></stages>` entries. Only the
/// most common stage fields are typed; use [`HediffStage::child`] /
/// [`HediffStage::raw`] for anything else (cap Mods, ability gains, ...).
#[derive(Debug, Clone)]
pub struct HediffStage {
    label: Option<String>,
    min_severity: Option<f32>,
    max_severity: Option<f32>,
    pain_offset: Option<f32>,
    become_visible: bool,
    extras: Vec<Node>,
}

impl HediffStage {
    pub fn new() -> Self {
        Self {
            label: None,
            min_severity: None,
            max_severity: None,
            pain_offset: None,
            become_visible: false,
            extras: Vec::new(),
        }
    }

    /// Overrides the def's label while the pawn is in this stage.
    pub fn label(mut self, label: impl Into<String>) -> Self {
        self.label = Some(label.into());
        self
    }

    pub fn min_severity(mut self, severity: f32) -> Self {
        self.min_severity = Some(severity);
        self
    }

    pub fn max_severity(mut self, severity: f32) -> Self {
        self.max_severity = Some(severity);
        self
    }

    pub fn pain_offset(mut self, pain: f32) -> Self {
        self.pain_offset = Some(pain);
        self
    }

    /// Makes the stage visible on the health card and in the UI.
    pub fn become_visible(mut self, value: bool) -> Self {
        self.become_visible = value;
        self
    }

    /// Sets `capMods` with one entry: a capacity and an offset, e.g.
    /// `("Consciousness", -0.15)`.
    pub fn cap_offset(mut self, capacity: impl Into<String>, offset: impl Into<String>) -> Self {
        self.extras.push(
            Element::new("capMods")
                .child(
                    Element::new("li")
                        .child_text("capacity", capacity)
                        .child_text("offset", offset),
                )
                .into(),
        );
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

    fn into_element(self) -> Element {
        let mut li = Element::new("li");

        if let Some(label) = &self.label {
            li = li.child_text("label", label);
        }
        if let Some(severity) = self.min_severity {
            li = li.child_text("minSeverity", fmt_num(severity));
        }
        if let Some(severity) = self.max_severity {
            li = li.child_text("maxSeverity", fmt_num(severity));
        }
        if let Some(pain) = self.pain_offset {
            li = li.child_text("painOffset", fmt_num(pain));
        }
        if self.become_visible {
            li = li.child_text("becomeVisible", "True");
        }

        for extra in self.extras {
            li = li.child(extra);
        }

        li
    }
}

impl Default for HediffStage {
    fn default() -> Self {
        Self::new()
    }
}

/// Typed builder for a `HediffDef`.
///
/// Only common fields are typed. Use [`HediffDef::child`] / [`HediffDef::raw`]
/// for anything else, e.g. `hediffClass` or statParts.
#[derive(Debug, Clone)]
pub struct HediffDef {
    def_name: String,
    parent: Option<String>,
    label: Option<String>,
    description: Option<String>,
    default_label_color: Option<String>,
    initial_severity: Option<f32>,
    max_severity: Option<f32>,
    tendable: Option<bool>,
    stages: Vec<HediffStage>,
    extras: Vec<Node>,
}

impl HediffDef {
    pub fn new(def_name: impl Into<String>) -> Self {
        Self {
            def_name: def_name.into(),
            parent: None,
            label: None,
            description: None,
            default_label_color: None,
            initial_severity: None,
            max_severity: None,
            tendable: None,
            stages: Vec::new(),
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

    /// Sets the color used for this hediff's health card label, e.g. `(1, 0.4, 0.4)`.
    pub fn default_label_color(mut self, color: impl Into<String>) -> Self {
        self.default_label_color = Some(color.into());
        self
    }

    pub fn initial_severity(mut self, severity: f32) -> Self {
        self.initial_severity = Some(severity);
        self
    }

    pub fn max_severity(mut self, severity: f32) -> Self {
        self.max_severity = Some(severity);
        self
    }

    pub fn tendable(mut self, value: bool) -> Self {
        self.tendable = Some(value);
        self
    }

    /// Adds a stage to `<stages>`.
    pub fn stage(mut self, stage: HediffStage) -> Self {
        self.stages.push(stage);
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
        let mut root = Element::new("HediffDef");
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
        if let Some(color) = &self.default_label_color {
            root = root.child_text("defaultLabelColor", color);
        }
        if let Some(severity) = self.initial_severity {
            root = root.child_text("initialSeverity", fmt_num(severity));
        }
        if let Some(severity) = self.max_severity {
            root = root.child_text("maxSeverity", fmt_num(severity));
        }
        if let Some(tendable) = self.tendable {
            root = root.child_text("tendable", bool_text(tendable));
        }

        if !self.stages.is_empty() {
            let mut stages = Element::new("stages");
            for stage in self.stages {
                stages = stages.child(stage.into_element());
            }
            root = root.child(stages);
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

fn bool_text(value: bool) -> &'static str {
    if value { "True" } else { "False" }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::defs::DefSet;
    use crate::error::Error;

    #[test]
    fn builds_a_hediff_def() {
        let def = HediffDef::new("RimRsFrostbite")
            .parent("InjuryBase")
            .label("rim-rs frostbite")
            .description("A <b>cold</b> injury & more")
            .default_label_color("(0.5, 0.8, 1)")
            .initial_severity(0.1)
            .max_severity(1.0)
            .tendable(true)
            .stage(
                HediffStage::new()
                    .label("minor")
                    .min_severity(0.0)
                    .max_severity(0.4)
                    .become_visible(true)
                    .cap_offset("Consciousness", "-0.05"),
            )
            .stage(HediffStage::new().label("serious").min_severity(0.4))
            .build();

        let xml = def.into_element().to_xml_string().unwrap();
        assert!(xml.contains(r#"<HediffDef ParentName="InjuryBase">"#));
        assert!(xml.contains("<defName>RimRsFrostbite</defName>"));
        assert!(xml.contains("<defaultLabelColor>(0.5, 0.8, 1)</defaultLabelColor>"));
        assert!(xml.contains("<initialSeverity>0.1</initialSeverity>"));
        assert!(xml.contains("<tendable>True</tendable>"));
        assert!(xml.contains("<stages>"));
        assert!(xml.contains("<label>minor</label>"));
        assert!(xml.contains("<maxSeverity>0.4</maxSeverity>"));
        assert!(xml.contains("<becomeVisible>True</becomeVisible>"));
        assert!(xml.contains("<capacity>Consciousness</capacity>"));
        assert!(xml.contains("<label>serious</label>"));
    }

    #[test]
    fn rejects_an_invalid_def_name() {
        let mut set = DefSet::new();
        set.add(HediffDef::new("has space").label("broken"));
        assert!(matches!(
            set.xml_files(),
            Err(Error::InvalidDefName(name)) if name == "has space"
        ));
    }
}
