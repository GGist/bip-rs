use std::collections::{BTreeMap};

/// Trait for working with generic map data structures.
pub trait Dictionary<'a, V> {
    /// Convert the dictionary to an unordered list of key/value pairs.
    fn to_list(&self) -> Vec<(&'a str, &V)>;

    /// Lookup a value in the dictionary.
    fn lookup(&self, key: &str) -> Option<&V>;
    
    /// Lookup a mutable value in the dictionary.
    fn lookup_mut(&mut self, key: &str) -> Option<&mut V>;

    /// Insert a key/value pair into the dictionary.
    fn insert(&mut self, key: &'a str, value: V) -> Option<V>;
    
    /// Remove a value from the dictionary and return it.
    fn remove(&mut self, key: &str) -> Option<V>;
}

impl<'a, V> Dictionary<'a, V> for BTreeMap<&'a str, V> {
    fn to_list(&self) -> Vec<(&'a str, &V)> {
        self.iter().map(|(k, v)| (*k, v)).collect()
    }

    fn lookup(&self, key: &str) -> Option<&V> {
        self.get(key)
    }
    
    fn lookup_mut(&mut self, key: &str) -> Option<&mut V> {
        self.get_mut(key)
    }

    fn insert(&mut self, key: &'a str, value: V) -> Option<V> {
        self.insert(key, value)
    }
    
    fn remove(&mut self, key: &str) -> Option<V> {
        self.remove(key)
    }
}