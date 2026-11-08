use crate::defs::Def;
use crate::xml::Element;

/// Typed builder for a `StuffCategoryDef` (e.g. a mod-internal
/// `Ham_BladeBillets` category used by custom intermediate materials).
#[derive(Debug, Clone, Default)]
pub struct StuffCategoryDef {
    def_name: String,
    label: Option<String>,
    list_order: Option<u32>,
}

impl StuffCategoryDef {
    pub fn new(def_name: impl Into<String>) -> Self {
        Self {
            def_name: def_name.into(),
            label: None,
            list_order: None,
        }
    }

    pub fn label(mut self, label: impl Into<String>) -> Self {
        self.label = Some(label.into());
        self
    }

    /// Sets `listOrder`, which orders the material class in UI lists. Should
    /// be unique among all stuff categories (vanilla uses multiples of 10).
    pub fn list_order(mut self, order: u32) -> Self {
        self.list_order = Some(order);
        self
    }

    pub fn build(self) -> Def {
        let mut root = Element::new("StuffCategoryDef").child_text("defName", &self.def_name);

        if let Some(label) = &self.label {
            root = root.child_text("label", label);
        }
        if let Some(order) = self.list_order {
            root = root.child_text("listOrder", order.to_string());
        }

        Def::from(root)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builds_a_stuff_category_def() {
        let def = StuffCategoryDef::new("Ham_BladeBillets")
            .label("blade billet")
            .list_order(420)
            .build();

        let xml = def.into_element().to_xml_string().unwrap();
        assert!(xml.contains("<StuffCategoryDef>"));
        assert!(xml.contains("<defName>Ham_BladeBillets</defName>"));
        assert!(xml.contains("<label>blade billet</label>"));
        assert!(xml.contains("<listOrder>420</listOrder>"));
    }
}
