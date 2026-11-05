use crate::defs::Def;
use crate::xml::{Element, Node};

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
    products: Vec<(String, u32)>,
    recipe_users: Vec<String>,
    extras: Vec<Node>,
}

#[derive(Debug, Clone)]
struct Ingredient {
    def_name: String,
    count: u32,
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
        self.ingredients.push(Ingredient {
            def_name: def_name.into(),
            count,
        });
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
                let filter = Element::new("filter")
                    .child(Element::new("thingDefs").child_text("li", &ingredient.def_name));
                let li = Element::new("li")
                    .child(filter)
                    .child_text("count", ingredient.count.to_string());
                ingredients = ingredients.child(li);
            }
            root = root.child(ingredients);
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
        assert!(xml.contains("<thingDefs>"));
        assert!(xml.contains("<li>Steel</li>"));
        assert!(xml.contains("<products>"));
        assert!(xml.contains("<MyThing>1</MyThing>"));
        assert!(xml.contains("<recipeUsers>"));
        assert!(xml.contains("<li>CraftingSpot</li>"));
    }
}
