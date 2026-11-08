use crate::error::{Error, Result};
use crate::xml::{Element, Node};

pub mod hediff;
pub mod recipe;
pub mod research;
pub mod stuff;
pub mod terrain;
pub mod thing;

pub use hediff::HediffDef;
pub use recipe::{RecipeDef, ThingFilter};
pub use research::ResearchProjectDef;
pub use stuff::StuffCategoryDef;
pub use terrain::TerrainDef;
pub use thing::ThingDef;

/// A generic RimWorld Def, backed by an XML element.
#[derive(Debug, Clone)]
pub struct Def {
    element: Element,
}

impl Def {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            element: Element::new(name),
        }
    }

    pub fn attr(mut self, name: impl Into<String>, value: impl Into<String>) -> Self {
        self.element = self.element.attr(name, value);
        self
    }

    pub fn child(mut self, child: impl Into<Node>) -> Self {
        self.element = self.element.child(child);
        self
    }

    pub fn child_text(mut self, name: impl Into<String>, value: impl Into<String>) -> Self {
        self.element = self.element.child_text(name, value);
        self
    }

    pub fn text(mut self, text: impl Into<String>) -> Self {
        self.element = self.element.text(text);
        self
    }

    /// Inserts a raw XML fragment.
    pub fn raw(mut self, raw: impl Into<String>) -> Self {
        self.element = self.element.raw(raw);
        self
    }

    pub fn element(&self) -> &Element {
        &self.element
    }

    pub fn into_element(self) -> Element {
        self.element
    }
}

impl From<Element> for Def {
    fn from(element: Element) -> Self {
        Self { element }
    }
}

impl From<Def> for Element {
    fn from(value: Def) -> Self {
        value.element
    }
}

/// Anything that can become a [`Def`].
///
/// Implement this for typed builders (`ThingDef`, `RecipeDef`, ...) so they can
/// be passed to [`DefSet::add`].
pub trait IntoDef {
    fn into_def(self) -> Def;
}

impl IntoDef for Def {
    fn into_def(self) -> Def {
        self
    }
}

impl IntoDef for Element {
    fn into_def(self) -> Def {
        Def::from(self)
    }
}

impl IntoDef for ThingDef {
    fn into_def(self) -> Def {
        self.build()
    }
}

impl IntoDef for RecipeDef {
    fn into_def(self) -> Def {
        self.build()
    }
}

impl IntoDef for HediffDef {
    fn into_def(self) -> Def {
        self.build()
    }
}

impl IntoDef for ResearchProjectDef {
    fn into_def(self) -> Def {
        self.build()
    }
}

impl IntoDef for StuffCategoryDef {
    fn into_def(self) -> Def {
        self.build()
    }
}

impl IntoDef for TerrainDef {
    fn into_def(self) -> Def {
        self.build()
    }
}

/// A collection of Defs, optionally grouped into multiple XML files.
#[derive(Debug, Default)]
pub struct DefSet {
    files: Vec<(String, Vec<Def>)>,
}

impl DefSet {
    pub const DEFAULT_FILE: &'static str = "Generated.xml";

    pub fn new() -> Self {
        Self::default()
    }

    /// Adds a Def to the default file (`Defs/Generated.xml`).
    pub fn add(&mut self, def: impl IntoDef) -> &mut Self {
        self.add_to(Self::DEFAULT_FILE, def)
    }

    /// Adds a Def to a named file. `name` is relative to `Defs/`.
    pub fn add_to(&mut self, file: impl Into<String>, def: impl IntoDef) -> &mut Self {
        let file = normalize_xml_file_name(file.into());
        match self.files.iter_mut().find(|(name, _)| name == &file) {
            Some((_, items)) => items.push(def.into_def()),
            None => self.files.push((file, vec![def.into_def()])),
        }
        self
    }

    /// Runs a closure that adds Defs to a named file.
    pub fn file(&mut self, name: impl Into<String>, f: impl FnOnce(&mut DefFile<'_>)) -> &mut Self {
        let name = normalize_xml_file_name(name.into());
        let index = match self.files.iter().position(|(file, _)| file == &name) {
            Some(index) => index,
            None => {
                self.files.push((name, Vec::new()));
                self.files.len() - 1
            }
        };

        let mut file = DefFile {
            items: &mut self.files[index].1,
        };
        f(&mut file);
        self
    }

    pub fn is_empty(&self) -> bool {
        self.files.iter().all(|(_, items)| items.is_empty())
    }

    /// Produces `(file_name, <Defs> root)` pairs ready for serialization.
    ///
    /// Validates every `defName` found in the set; see [`validate_def_name`].
    pub fn xml_files(&self) -> Result<Vec<(String, Element)>> {
        for file in &self.files {
            for def in &file.1 {
                check_def_name(def)?;
            }
        }

        let files: Vec<(String, Element)> = self
            .files
            .iter()
            .filter(|(_, items)| !items.is_empty())
            .map(|(name, items)| {
                let mut root = Element::new("Defs");
                for def in items {
                    root = root.child(def.element.clone());
                }
                (name.clone(), root)
            })
            .collect();
        Ok(files)
    }
}

/// Mutable view passed to [`DefSet::file`].
pub struct DefFile<'a> {
    items: &'a mut Vec<Def>,
}

impl DefFile<'_> {
    pub fn add(&mut self, def: impl IntoDef) -> &mut Self {
        self.items.push(def.into_def());
        self
    }

    pub fn add_all<I>(&mut self, defs: I) -> &mut Self
    where
        I: IntoIterator,
        I::Item: IntoDef,
    {
        for def in defs {
            self.items.push(def.into_def());
        }
        self
    }
}

fn normalize_xml_file_name(name: String) -> String {
    if name.ends_with(".xml") {
        name
    } else {
        format!("{name}.xml")
    }
}

/// RimWorld `defName`s are referenced from XML, C# (`DefOf`) and save files.
/// Anything outside ASCII letters/digits/`_`/`-`/`.` is rejected early instead
/// of failing at game load time.
pub fn validate_def_name(def_name: &str) -> Result<()> {
    let valid = !def_name.is_empty()
        && def_name.chars().all(|character| {
            character.is_ascii_alphanumeric() || matches!(character, '_' | '-' | '.')
        });

    if valid {
        Ok(())
    } else {
        Err(Error::InvalidDefName(def_name.to_string()))
    }
}

/// Validates the `defName` child of a Def, when the Def has one.
///
/// Abstract parents (`<ThingDef Name="..." Abstract="True">`) have no
/// `defName` and are accepted as-is.
fn check_def_name(def: &Def) -> Result<()> {
    for node in &def.element.children {
        if let Node::Element(element) = node
            && element.name == "defName"
            && let Some(Node::Text(text)) = element.children.first()
        {
            validate_def_name(text)?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn groups_defs_by_file() {
        let mut set = DefSet::new();
        set.add(Def::new("ThingDef").child_text("defName", "A"));
        set.file("Things", |file| {
            file.add(Def::new("ThingDef").child_text("defName", "B"));
        });

        let files = set.xml_files().unwrap();
        assert_eq!(files.len(), 2);
        assert_eq!(files[0].0, "Generated.xml");
        assert_eq!(files[1].0, "Things.xml");
    }

    #[test]
    fn accepts_valid_def_names() {
        for name in ["A", "RimRsPurpleCube", "Make_X", "item-1", "a.b_2"] {
            assert!(validate_def_name(name).is_ok(), "{name}");
        }
    }

    #[test]
    fn rejects_invalid_def_names() {
        for name in ["", "has space", "不", "a\tb", "$x"] {
            assert!(validate_def_name(name).is_err(), "{name}");
        }
    }

    #[test]
    fn xml_files_validates_def_names() {
        let mut set = DefSet::new();
        set.add(Def::new("ThingDef").child_text("defName", "bad name"));
        assert!(matches!(
            set.xml_files(),
            Err(Error::InvalidDefName(name)) if name == "bad name"
        ));
    }

    #[test]
    fn abstract_defs_without_def_name_pass() {
        let mut set = DefSet::new();
        set.add(
            Def::new("ThingDef")
                .attr("Name", "Base")
                .attr("Abstract", "True"),
        );
        assert!(set.xml_files().is_ok());
    }
}
