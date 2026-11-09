use crate::defs::Def;
use crate::xml::{Element, Node};

/// An ingredient filter (`ThingFilter`) for a recipe.
///
/// Field names are verified against vanilla 1.6 data (`Recipes_Food.xml`;
/// Faction_Empire apparel filters are the same shape): `thingDefs`,
/// `categories`, `stuffCategoriesToAllow`, `disallowedThingDefs`,
/// `disallowedCategories`.
///
/// ```no_run
/// use rim_gen::prelude::*;
///
/// RecipeDef::new("Make_Billet")
///     .ingredient_stuff(["Metallic"], 2)
///     .default_ingredient_filter(|f| f.disallow_thing(["Gold"]));
/// ```
#[derive(Debug, Clone, Default)]
pub struct ThingFilter {
    things: Vec<String>,
    categories: Vec<String>,
    stuff_categories: Vec<String>,
    disallowed_things: Vec<String>,
    disallowed_categories: Vec<String>,
}

impl ThingFilter {
    pub fn new() -> Self {
        Self::default()
    }

    /// Allows specific thing defs (`<thingDefs>`).
    pub fn allow_thing<I, S>(mut self, def_names: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: AsRef<str>,
    {
        self.things
            .extend(def_names.into_iter().map(|item| item.as_ref().to_string()));
        self
    }

    /// Allows specific thing categories (`<categories>`).
    pub fn allow_category<I, S>(mut self, categories: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: AsRef<str>,
    {
        self.categories
            .extend(categories.into_iter().map(|item| item.as_ref().to_string()));
        self
    }

    /// Allows everything made from the listed stuff categories
    /// (`<stuffCategoriesToAllow>`), e.g. `Metallic` for "any metal".
    pub fn allow_stuff_category<I, S>(mut self, categories: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: AsRef<str>,
    {
        self.stuff_categories
            .extend(categories.into_iter().map(|item| item.as_ref().to_string()));
        self
    }

    /// Disallows specific thing defs (`<disallowedThingDefs>`, vanilla shape).
    pub fn disallow_thing<I, S>(mut self, def_names: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: AsRef<str>,
    {
        self.disallowed_things
            .extend(def_names.into_iter().map(|item| item.as_ref().to_string()));
        self
    }

    /// Disallows specific thing categories (`<disallowedCategories>`).
    pub fn disallow_category<I, S>(mut self, categories: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: AsRef<str>,
    {
        self.disallowed_categories
            .extend(categories.into_iter().map(|item| item.as_ref().to_string()));
        self
    }

    /// Renders the filter as an element named `tag_name` (the tag itself is a
    /// field name of the enclosing def, so callers pick it: `filter`,
    /// `fixedIngredientFilter`, `defaultIngredientFilter`, ...).
    fn render(self, tag_name: &str) -> Element {
        let mut element = Element::new(tag_name);

        if !self.things.is_empty() {
            let mut list = Element::new("thingDefs");
            for def_name in &self.things {
                list = list.child_text("li", def_name);
            }
            element = element.child(list);
        }
        if !self.categories.is_empty() {
            let mut list = Element::new("categories");
            for category in &self.categories {
                list = list.child_text("li", category);
            }
            element = element.child(list);
        }
        if !self.stuff_categories.is_empty() {
            let mut list = Element::new("stuffCategoriesToAllow");
            for category in &self.stuff_categories {
                list = list.child_text("li", category);
            }
            element = element.child(list);
        }
        if !self.disallowed_things.is_empty() {
            let mut list = Element::new("disallowedThingDefs");
            for def_name in &self.disallowed_things {
                list = list.child_text("li", def_name);
            }
            element = element.child(list);
        }
        if !self.disallowed_categories.is_empty() {
            let mut list = Element::new("disallowedCategories");
            for category in &self.disallowed_categories {
                list = list.child_text("li", category);
            }
            element = element.child(list);
        }

        element
    }
}

/// One ingredient: anything matched by `filter`, in `count`.
#[derive(Debug, Clone)]
struct Ingredient {
    filter: ThingFilter,
    count: u32,
}

/// Typed builder for a `RecipeDef`.
#[derive(Debug, Clone)]
pub struct RecipeDef {
    def_name: String,
    parent: Option<String>,
    label: Option<String>,
    description: Option<String>,
    job_string: Option<String>,
    work_amount: Option<f32>,
    work_speed_stat: Option<String>,
    work_skill: Option<String>,
    ingredients: Vec<Ingredient>,
    allow_mixing_ingredients: Option<bool>,
    fixed_ingredient_filter: Option<ThingFilter>,
    default_ingredient_filter: Option<ThingFilter>,
    products: Vec<(String, u32)>,
    recipe_users: Vec<String>,
    extras: Vec<Node>,
}

impl RecipeDef {
    pub fn new(def_name: impl Into<String>) -> Self {
        Self {
            def_name: def_name.into(),
            parent: None,
            label: None,
            description: None,
            job_string: None,
            work_amount: None,
            work_speed_stat: None,
            work_skill: None,
            ingredients: Vec::new(),
            allow_mixing_ingredients: None,
            fixed_ingredient_filter: None,
            default_ingredient_filter: None,
            products: Vec::new(),
            recipe_users: Vec::new(),
            extras: Vec::new(),
        }
    }

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

    pub fn job_string(mut self, job_string: impl Into<String>) -> Self {
        self.job_string = Some(job_string.into());
        self
    }

    pub fn work_amount(mut self, amount: f32) -> Self {
        self.work_amount = Some(amount);
        self
    }

    pub fn work_speed_stat(mut self, stat: impl Into<String>) -> Self {
        self.work_speed_stat = Some(stat.into());
        self
    }

    pub fn work_skill(mut self, skill: impl Into<String>) -> Self {
        self.work_skill = Some(skill.into());
        self
    }

    /// Adds an ingredient filtered by a single thingDef.
    pub fn ingredient(mut self, def_name: impl Into<String>, count: u32) -> Self {
        let def_name = def_name.into();
        self.ingredients.push(Ingredient {
            filter: ThingFilter::new().allow_thing([def_name.as_str()]),
            count,
        });
        self
    }

    /// Adds a material-class ingredient: anything whose def is stuff of the
    /// listed categories (`filter -> stuffCategoriesToAllow`), e.g.
    /// `.ingredient_stuff(["Metallic"], 2)` or
    /// `.ingredient_stuff(["Woody", "Metallic"], 4)`.
    pub fn ingredient_stuff<I, S>(mut self, categories: I, count: u32) -> Self
    where
        I: IntoIterator<Item = S>,
        S: AsRef<str>,
    {
        self.ingredients.push(Ingredient {
            filter: ThingFilter::new().allow_stuff_category(categories),
            count,
        });
        self
    }

    /// Sets `allowMixingIngredients` (vanilla default `false`): whether one
    /// ingredient line may be filled with a mix of different materials.
    pub fn allow_mixing_ingredients(mut self, value: bool) -> Self {
        self.allow_mixing_ingredients = Some(value);
        self
    }

    /// Sets the bill's overall allowed pool (`fixedIngredientFilter`).
    pub fn fixed_ingredient_filter(mut self, f: impl FnOnce(ThingFilter) -> ThingFilter) -> Self {
        self.fixed_ingredient_filter = Some(f(ThingFilter::new()));
        self
    }

    /// Narrows what bills suggest by default (`defaultIngredientFilter`).
    pub fn default_ingredient_filter(mut self, f: impl FnOnce(ThingFilter) -> ThingFilter) -> Self {
        self.default_ingredient_filter = Some(f(ThingFilter::new()));
        self
    }

    /// Adds a product. RimWorld's `<products>` is a defName -> count map.
    pub fn product(mut self, def_name: impl Into<String>, count: u32) -> Self {
        self.products.push((def_name.into(), count));
        self
    }

    /// Adds a bench/thing that can use this recipe.
    pub fn recipe_user(mut self, def_name: impl Into<String>) -> Self {
        self.recipe_users.push(def_name.into());
        self
    }

    pub fn child(mut self, child: impl Into<Node>) -> Self {
        self.extras.push(child.into());
        self
    }

    pub fn raw(mut self, raw: impl Into<String>) -> Self {
        self.extras.push(Node::Raw(raw.into()));
        self
    }

    pub fn build(self) -> Def {
        let mut root = Element::new("RecipeDef");
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
        if let Some(job_string) = &self.job_string {
            root = root.child_text("jobString", job_string);
        }
        if let Some(amount) = self.work_amount {
            root = root.child_text("workAmount", fmt_num(amount));
        }
        if let Some(stat) = &self.work_speed_stat {
            root = root.child_text("workSpeedStat", stat);
        }
        if let Some(skill) = &self.work_skill {
            root = root.child_text("workSkill", skill);
        }

        if !self.ingredients.is_empty() {
            let mut ingredients = Element::new("ingredients");
            for ingredient in &self.ingredients {
                let filter = ingredient.filter.clone().render("filter");
                let li = Element::new("li")
                    .child(filter)
                    .child_text("count", ingredient.count.to_string());
                ingredients = ingredients.child(li);
            }
            root = root.child(ingredients);
        }

        if let Some(value) = self.allow_mixing_ingredients {
            root = root.child_text(
                "allowMixingIngredients",
                if value { "true" } else { "false" },
            );
        }

        if let Some(filter) = self.fixed_ingredient_filter {
            root = root.child(filter.render("fixedIngredientFilter"));
        }
        if let Some(filter) = self.default_ingredient_filter {
            root = root.child(filter.render("defaultIngredientFilter"));
        }

        if !self.products.is_empty() {
            let mut products = Element::new("products");
            for (def_name, count) in &self.products {
                products = products.child_text(def_name, count.to_string());
            }
            root = root.child(products);
        }

        if !self.recipe_users.is_empty() {
            let mut users = Element::new("recipeUsers");
            for user in &self.recipe_users {
                users = users.child_text("li", user);
            }
            root = root.child(users);
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
    fn builds_a_recipe() {
        let def = RecipeDef::new("Make_MyThing")
            .label("make my thing")
            .work_amount(120.0)
            .ingredient("Steel", 20)
            .product("MyThing", 1)
            .recipe_user("CraftingSpot")
            .build();

        let xml = def.into_element().to_xml_string().unwrap();
        assert!(xml.contains("<defName>Make_MyThing</defName>"));
        assert!(xml.contains("<workAmount>120</workAmount>"));
        let xml = xml.replace(['\n', '\t'], "");
        assert!(xml.contains("<filter><thingDefs><li>Steel</li></thingDefs></filter>"));
        assert!(xml.contains("<products>"));
        assert!(xml.contains("<MyThing>1</MyThing>"));
        assert!(xml.contains("<recipeUsers>"));
        assert!(xml.contains("<li>CraftingSpot</li>"));
    }

    #[test]
    fn builds_a_material_class_recipe() {
        let def = RecipeDef::new("Make_Billet")
            .ingredient_stuff(["Metallic"], 2)
            .fixed_ingredient_filter(|f| f.allow_stuff_category(["Metallic"]))
            .default_ingredient_filter(|f| f.disallow_thing(["Gold"]))
            .build();

        let xml = def.into_element().to_xml_string().unwrap();
        let xml = xml.replace(['\n', '\t'], "");
        assert!(xml.contains(
            "<filter><stuffCategoriesToAllow><li>Metallic</li></stuffCategoriesToAllow></filter>"
        ));
        assert!(xml.contains("<count>2</count>"));
        assert!(xml.contains(
            "<fixedIngredientFilter><stuffCategoriesToAllow><li>Metallic</li>\
             </stuffCategoriesToAllow></fixedIngredientFilter>"
        ));
        assert!(xml.contains(
            "<defaultIngredientFilter><disallowedThingDefs><li>Gold</li></disallowedThingDefs>\
             </defaultIngredientFilter>"
        ));
    }

    #[test]
    fn emits_allow_mixing_ingredients() {
        let def = RecipeDef::new("Make_Mixed")
            .ingredient_stuff(["Metallic"], 3)
            .ingredient_stuff(["Woody"], 3)
            .allow_mixing_ingredients(true)
            .build();

        let xml = def.into_element().to_xml_string().unwrap();
        assert!(xml.contains("<allowMixingIngredients>true</allowMixingIngredients>"));
        assert_eq!(xml.matches("<stuffCategoriesToAllow>").count(), 2);
    }
}
