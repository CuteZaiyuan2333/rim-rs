use crate::defs::Def;
use crate::xml::{Element, Node};

/// Typed builder for a `ThingDef`.
///
/// Only common fields are typed. Use [`ThingDef::child`] / [`ThingDef::raw`] for
/// anything else.
///
/// The two "stuff" concepts have distinct helpers (RimWorld's two unrelated
/// fields): [`ThingDef::made_from_stuff`] writes `stuffCategories` (the def is
/// made from stuff), [`ThingDef::is_stuff`] writes `stuffProps/categories`
/// (the def itself is a stuff).
#[derive(Debug, Clone)]
pub struct ThingDef {
    def_name: String,
    parent: Option<String>,
    name: Option<String>,
    is_abstract: bool,
    label: Option<String>,
    description: Option<String>,
    thing_class: Option<String>,
    category: Option<String>,
    size: Option<(f32, f32)>,
    stack_limit: Option<u32>,
    graphic: Option<GraphicData>,
    pending_graphic_class: Option<String>,
    pending_draw_size: Option<(f32, f32)>,
    stat_bases: Vec<(String, String)>,
    stat_factors: Vec<(String, String)>,
    stuff_categories: Vec<String>,
    is_stuff: Vec<String>,
    use_hit_points: Option<bool>,
    health_affects_price: Option<bool>,
    comps: Vec<Element>,
    extras: Vec<Node>,
}

#[derive(Debug, Clone)]
struct GraphicData {
    tex_path: String,
    graphic_class: String,
    draw_size: Option<(f32, f32)>,
}

impl ThingDef {
    pub fn new(def_name: impl Into<String>) -> Self {
        Self {
            def_name: def_name.into(),
            parent: None,
            name: None,
            is_abstract: false,
            label: None,
            description: None,
            thing_class: None,
            category: None,
            size: None,
            stack_limit: None,
            graphic: None,
            pending_graphic_class: None,
            pending_draw_size: None,
            stat_bases: Vec::new(),
            stat_factors: Vec::new(),
            stuff_categories: Vec::new(),
            is_stuff: Vec::new(),
            use_hit_points: None,
            health_affects_price: None,
            comps: Vec::new(),
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

    /// Sets the `Name` attribute (used by abstract base defs).
    pub fn named(mut self, name: impl Into<String>) -> Self {
        self.name = Some(name.into());
        self
    }

    pub fn is_abstract(mut self, value: bool) -> Self {
        self.is_abstract = value;
        self
    }

    /// Declares a mod-internal abstract base def:
    /// `Name` attribute, `ParentName` attribute, `Abstract="True"`, and no
    /// `defName` (abstract templates must not export one).
    pub fn abstract_parent(mut self, name: impl Into<String>, parent: impl Into<String>) -> Self {
        self.name = Some(name.into());
        self.parent = Some(parent.into());
        self.is_abstract = true;
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

    pub fn thing_class(mut self, class: impl Into<String>) -> Self {
        self.thing_class = Some(class.into());
        self
    }

    pub fn category(mut self, category: impl Into<String>) -> Self {
        self.category = Some(category.into());
        self
    }

    pub fn size(mut self, width: f32, height: f32) -> Self {
        self.size = Some((width, height));
        self
    }

    pub fn stack_limit(mut self, limit: u32) -> Self {
        self.stack_limit = Some(limit);
        self
    }

    /// Sets `<graphicData><texPath>...`.
    ///
    /// Defaults `graphicClass` to `Graphic_Single`; override it with
    /// [`ThingDef::graphic_class`].
    pub fn texture(mut self, tex_path: impl Into<String>) -> Self {
        let graphic_class = self
            .pending_graphic_class
            .take()
            .unwrap_or_else(|| "Graphic_Single".to_string());
        self.graphic = Some(GraphicData {
            tex_path: tex_path.into(),
            graphic_class,
            draw_size: self.pending_draw_size.take(),
        });
        self
    }

    pub fn graphic_class(mut self, class: impl Into<String>) -> Self {
        let class = class.into();
        match &mut self.graphic {
            Some(graphic) => graphic.graphic_class = class,
            None => self.pending_graphic_class = Some(class),
        }
        self
    }

    pub fn draw_size(mut self, width: f32, height: f32) -> Self {
        match &mut self.graphic {
            Some(graphic) => graphic.draw_size = Some((width, height)),
            None => self.pending_draw_size = Some((width, height)),
        }
        self
    }

    pub fn stat_base(mut self, name: impl Into<String>, value: impl ToString) -> Self {
        self.stat_bases.push((name.into(), value.to_string()));
        self
    }

    pub fn stat_factor(mut self, name: impl Into<String>, value: impl ToString) -> Self {
        self.stat_factors.push((name.into(), value.to_string()));
        self
    }

    /// Marks the def as made from the listed stuff categories
    /// (`<stuffCategories>`).
    pub fn made_from_stuff<I, S>(mut self, categories: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: AsRef<str>,
    {
        self.stuff_categories = categories
            .into_iter()
            .map(|category| category.as_ref().to_string())
            .collect();
        self
    }

    /// Marks the def itself as a stuff of the listed stuff categories
    /// (`<stuffProps><categories>`). See [`ThingDef::made_from_stuff`] for the
    /// other direction.
    pub fn is_stuff<I, S>(mut self, categories: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: AsRef<str>,
    {
        self.is_stuff = categories
            .into_iter()
            .map(|category| category.as_ref().to_string())
            .collect();
        self
    }

    /// Sets `<useHitPoints>`. Vanilla default is `true`; pass `false` for
    /// raw-resource intermediates that should not degrade per material.
    pub fn use_hit_points(mut self, value: bool) -> Self {
        self.use_hit_points = Some(value);
        self
    }

    /// Sets `<healthAffectsPrice>`. Vanilla default is `true`.
    pub fn health_affects_price(mut self, value: bool) -> Self {
        self.health_affects_price = Some(value);
        self
    }

    pub fn market_value(self, value: impl ToString) -> Self {
        self.stat_base("MarketValue", value)
    }

    pub fn mass(self, value: impl ToString) -> Self {
        self.stat_base("Mass", value)
    }

    pub fn hit_points(self, value: impl ToString) -> Self {
        self.stat_base("MaxHitPoints", value)
    }

    /// Adds a `<li>` to `<comps>` with the given `Class` attribute.
    pub fn comp_class(mut self, class: impl Into<String>, f: impl FnOnce(&mut Element)) -> Self {
        let mut li = Element::new("li").attr("Class", class);
        f(&mut li);
        self.comps.push(li);
        self
    }

    /// Adds a raw `<li>` element to `<comps>`.
    pub fn comp(mut self, comp: Element) -> Self {
        self.comps.push(comp);
        self
    }

    /// Adds an arbitrary child element/raw node.
    pub fn child(mut self, child: impl Into<Node>) -> Self {
        self.extras.push(child.into());
        self
    }

    pub fn raw(mut self, raw: impl Into<String>) -> Self {
        self.extras.push(Node::Raw(raw.into()));
        self
    }

    /// Builds the underlying [`Def`].
    pub fn build(self) -> Def {
        let mut root = Element::new("ThingDef");

        if let Some(parent) = &self.parent {
            root = root.attr("ParentName", parent);
        }
        if let Some(name) = &self.name {
            root = root.attr("Name", name);
        }
        if self.is_abstract {
            root = root.attr("Abstract", "True");
        }

        // Abstract templates (`Name`/`ParentName` inheritance anchors) must
        // not export a `defName`; `DefSet` skips name checks for defs without
        // one. A named concrete def still needs the defName it was given.
        if self.name.is_none() || !self.def_name.is_empty() {
            root = root.child_text("defName", &self.def_name);
        }

        if let Some(label) = &self.label {
            root = root.child_text("label", label);
        }
        if let Some(description) = &self.description {
            root = root.child_text("description", description);
        }
        if let Some(thing_class) = &self.thing_class {
            root = root.child_text("thingClass", thing_class);
        }
        if let Some(category) = &self.category {
            root = root.child_text("category", category);
        }
        if let Some((width, height)) = self.size {
            root = root.child_text("size", format!("({},{})", fmt_num(width), fmt_num(height)));
        }
        if let Some(limit) = self.stack_limit {
            root = root.child_text("stackLimit", limit.to_string());
        }

        if !self.stuff_categories.is_empty() {
            let mut element = Element::new("stuffCategories");
            for name in &self.stuff_categories {
                element = element.child_text("li", name);
            }
            root = root.child(element);
        }

        if !self.is_stuff.is_empty() {
            let mut categories = Element::new("categories");
            for name in &self.is_stuff {
                categories = categories.child_text("li", name);
            }
            root = root.child(Element::new("stuffProps").child(categories));
        }

        if let Some(value) = self.use_hit_points {
            root = root.child_text("useHitPoints", if value { "true" } else { "false" });
        }
        if let Some(value) = self.health_affects_price {
            root = root.child_text("healthAffectsPrice", if value { "true" } else { "false" });
        }

        if let Some(graphic) = &self.graphic {
            let mut element = Element::new("graphicData")
                .child_text("texPath", &graphic.tex_path)
                .child_text("graphicClass", &graphic.graphic_class);
            if let Some((width, height)) = graphic.draw_size {
                element = element.child_text(
                    "drawSize",
                    format!("({},{})", fmt_num(width), fmt_num(height)),
                );
            }
            root = root.child(element);
        }

        if !self.stat_bases.is_empty() {
            let mut element = Element::new("statBases");
            for (name, value) in &self.stat_bases {
                element = element.child_text(name, value);
            }
            root = root.child(element);
        }

        if !self.stat_factors.is_empty() {
            let mut element = Element::new("statFactors");
            for (name, value) in &self.stat_factors {
                element = element.child_text(name, value);
            }
            root = root.child(element);
        }

        if !self.comps.is_empty() {
            let mut element = Element::new("comps");
            for comp in &self.comps {
                element = element.child(comp.clone());
            }
            root = root.child(element);
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

    #[test]
    fn builds_a_thing_def() {
        let def = ThingDef::new("MyThing")
            .parent("BaseThing")
            .label("my thing")
            .description("A <b>bold</b> thing & more")
            .market_value(42.0)
            .mass(0.5)
            .texture("Things/MyThing")
            .build();

        let xml = def.into_element().to_xml_string().unwrap();
        assert!(xml.contains(r#"<ThingDef ParentName="BaseThing">"#));
        assert!(xml.contains("<defName>MyThing</defName>"));
        assert!(xml.contains("&lt;b&gt;bold&lt;/b&gt;"));
        assert!(xml.contains("&amp; more"));
        assert!(xml.contains("<MarketValue>42</MarketValue>"));
        assert!(xml.contains("<texPath>Things/MyThing</texPath>"));
        assert!(xml.contains("<graphicClass>Graphic_Single</graphicClass>"));
    }

    #[test]
    fn builds_abstract_parents_without_def_name() {
        let def = ThingDef::new("")
            .abstract_parent("Kat_BaseMeleeWeapon", "ThingWithComps")
            .label("abstract base")
            .build();

        let xml = def.into_element().to_xml_string().unwrap();
        assert!(xml
            .contains(r#"<ThingDef ParentName="ThingWithComps" Name="Kat_BaseMeleeWeapon" Abstract="True">"#));
        assert!(!xml.contains("defName"));
    }

    #[test]
    fn keeps_a_def_name_when_named_concrete() {
        let def = ThingDef::new("MyNamedDef").named("MyNameAnchor").build();
        let xml = def.into_element().to_xml_string().unwrap();
        assert!(xml.contains(r#"<ThingDef Name="MyNameAnchor">"#));
        assert!(xml.contains("<defName>MyNamedDef</defName>"));
    }

    #[test]
    fn separates_made_from_stuff_from_is_stuff() {
        let def = ThingDef::new("MyThing")
            .made_from_stuff(["Metallic"])
            .is_stuff(["Ham_BladeBillets"])
            .use_hit_points(false)
            .health_affects_price(false)
            .build();

        let xml = def.into_element().to_xml_string().unwrap();
        let xml = xml.replace(['\n', '\t'], "");
        assert!(xml.contains("<stuffCategories><li>Metallic</li></stuffCategories>"));
        assert!(xml
            .contains("<stuffProps><categories><li>Ham_BladeBillets</li></categories></stuffProps>"));
        assert!(!xml.contains("stuffCategoriesToAllow"));
        assert!(xml.contains("<useHitPoints>false</useHitPoints>"));
        assert!(xml.contains("<healthAffectsPrice>false</healthAffectsPrice>"));
    }
}
