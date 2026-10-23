//! Knowledge (R-042). A colony knows a recipe from the start, or it learns the recipe from a
//! lore fragment. A recipe with a `requires` key in `recipes.ron` cannot be ordered until the
//! colony knows that key.

use crate::Sim;

impl Sim {
    /// The knowledge keys that the colony has learned, in key order.
    pub fn knowledge(&self) -> impl Iterator<Item = &str> + '_ {
        self.knowledge.iter().map(String::as_str)
    }

    /// True if the colony knows the key.
    pub fn knows(&self, key: &str) -> bool {
        self.knowledge.contains(key)
    }

    /// True if the colony can order the recipe: it has no lock, or the colony knows its key.
    pub fn recipe_known(&self, recipe: u16) -> bool {
        self.rules
            .recipe(recipe)
            .is_some_and(|r| r.requires.as_deref().is_none_or(|key| self.knows(key)))
    }

    /// The colony learns a key. Return false if it knew the key already.
    pub(crate) fn learn(&mut self, key: &str) -> bool {
        self.knowledge.insert(key.to_owned())
    }

    /// Give the colony a key. For scenario tests and debug tools.
    pub fn debug_learn(&mut self, key: &str) {
        self.learn(key);
    }
}
