use crate::error::{Error, Result};
use crate::xml::{Element, Node};

/// A PatchOperations file.
#[derive(Debug, Clone)]
pub struct Patch {
    name: String,
    operations: Vec<Operation>,
}

impl Patch {
    /// `name` becomes `Patches/<name>.xml`.
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            operations: Vec::new(),
        }
    }

    #[allow(clippy::should_implement_trait)]
    pub fn add(mut self, operation: Operation) -> Self {
        self.operations.push(operation);
        self
    }

    /// Adds an operation and configures it with a closure.
    pub fn op(mut self, class: impl Into<String>, f: impl FnOnce(&mut Operation)) -> Self {
        let mut operation = Operation::new(class);
        f(&mut operation);
        self.operations.push(operation);
        self
    }

    pub fn into_element(self) -> Element {
        let mut root = Element::new("Patch");
        for operation in self.operations {
            root = root.child(operation.into_element());
        }
        root
    }

    pub fn file_name(&self) -> String {
        if self.name.ends_with(".xml") {
            self.name.clone()
        } else {
            format!("{}.xml", self.name)
        }
    }
}

/// A single PatchOperation.
#[derive(Debug, Clone)]
pub struct Operation {
    class: String,
    xpath: Option<String>,
    value: Option<Element>,
    extras: Vec<Node>,
}

impl Operation {
    pub fn new(class: impl Into<String>) -> Self {
        Self {
            class: class.into(),
            xpath: None,
            value: None,
            extras: Vec::new(),
        }
    }

    pub fn xpath(&mut self, xpath: impl Into<String>) -> &mut Self {
        self.xpath = Some(xpath.into());
        self
    }

    pub fn value(&mut self, value: Element) -> &mut Self {
        self.value = Some(Element::new("value").child(value));
        self
    }

    /// Sets `<value>` to a raw XML fragment.
    pub fn raw_value(&mut self, raw: impl Into<String>) -> &mut Self {
        self.value = Some(Element::new("value").raw(raw));
        self
    }

    pub fn child(&mut self, child: impl Into<Node>) -> &mut Self {
        self.extras.push(child.into());
        self
    }

    pub fn raw(&mut self, raw: impl Into<String>) -> &mut Self {
        self.extras.push(Node::Raw(raw.into()));
        self
    }

    pub fn into_element(self) -> Element {
        let mut root = Element::new("Operation").attr("Class", self.class);

        if let Some(xpath) = self.xpath {
            root = root.child_text("xpath", xpath);
        }
        if let Some(value) = self.value {
            root = root.child(value);
        }
        for extra in self.extras {
            root = root.child(extra);
        }

        root
    }
}

/// A collection of patch files.
#[derive(Debug, Default)]
pub struct PatchSet {
    patches: Vec<Patch>,
}

impl PatchSet {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn add(&mut self, patch: Patch) -> &mut Self {
        self.patches.push(patch);
        self
    }

    pub fn is_empty(&self) -> bool {
        self.patches.is_empty()
    }

    /// Rejects empty patch files and operations without `xpath`/`value`/content,
    /// which would produce well-formed XML that RimWorld cannot apply.
    pub(crate) fn validate(&self) -> Result<()> {
        for patch in &self.patches {
            if patch.operations.is_empty() {
                return Err(Error::EmptyPatch(patch.file_name()));
            }
            for operation in &patch.operations {
                let is_empty = operation.xpath.is_none()
                    && operation.value.is_none()
                    && operation.extras.is_empty();
                if is_empty {
                    return Err(Error::EmptyOperation(operation.class.clone()));
                }
            }
        }
        Ok(())
    }

    pub fn xml_files(&self) -> Vec<(String, Element)> {
        self.patches
            .iter()
            .cloned()
            .map(|patch| (patch.file_name(), patch.into_element()))
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builds_a_patch_operation() {
        let patch = Patch::new("MyMod.Patches").op("PatchOperationAdd", |op| {
            op.xpath(r#"Defs/ThingDef[defName="Gun"]/statBases"#)
                .value(Element::new("MarketValue").text("200"));
        });

        let xml = patch.into_element().to_xml_string().unwrap();
        assert!(xml.contains(r#"<Operation Class="PatchOperationAdd">"#));
        assert!(xml.contains("<xpath>Defs/ThingDef[defName=&quot;Gun&quot;]/statBases</xpath>"));
        assert!(xml.contains("<value>"));
        assert!(xml.contains("<MarketValue>200</MarketValue>"));
    }

    #[test]
    fn rejects_an_empty_patch() {
        let set = PatchSet {
            patches: vec![Patch::new("Empty.Patches")],
        };
        assert!(
            matches!(set.validate(), Err(Error::EmptyPatch(name)) if name == "Empty.Patches.xml")
        );
    }

    #[test]
    fn rejects_an_operation_without_content() {
        let set = PatchSet {
            patches: vec![Patch::new("Patches").add(Operation::new("PatchOperationAdd"))],
        };
        assert!(
            matches!(set.validate(), Err(Error::EmptyOperation(class)) if class == "PatchOperationAdd")
        );
    }

    #[test]
    fn accepts_a_fully_configured_operation() {
        let set = PatchSet {
            patches: vec![Patch::new("Patches").op("PatchOperationAdd", |op| {
                op.xpath("Defs/ThingDef").value(Element::new("x").text("1"));
            })],
        };
        assert!(set.validate().is_ok());
    }
}
