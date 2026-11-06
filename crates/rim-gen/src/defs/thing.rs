use crate::defs::Def;
use crate::xml::{Element, Node};

/// Typed builder for a `ThingDef`.
///
/// Only common fields are typed. Use [`ThingDef::child`] / [`ThingDef::raw`] for
/// anything else.
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

        root = root.child_text("defName", &self.def_name);

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
}
