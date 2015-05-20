//! Utilities used throughout the library.

use std::borrow::{Borrow};
use std::collections::{HashMap, BTreeMap};
use std::hash::{Hash};

use rand;

/// Applies a Fisher-Yates shuffle on the given list.
pub fn fisher_shuffle<T: Copy>(list: &mut [T]) {
    for i in 0..list.len() {
        let swap_index = (rand::random::<usize>() % (list.len() - i)) + i;
        
        let temp = list[i];
        list[i] = list[swap_index];
        list[swap_index] = temp;
    }
}

//----------------------------------------------------------------------------//

/// Trait for working with generic map data structures.
pub trait Dictionary<K, V> where K: Borrow<str> {
    /// Convert the dictionary to an unordered list of key/value pairs.
    fn to_list<'a>(&'a self) -> Vec<(&'a K, &'a V)>;

    /// Lookup a value in the dictionary.
    fn lookup<'a>(&'a self, key: &str) -> Option<&'a V>;

    /// Insert a key/value pair into the dictionary.
    fn insert(&mut self, key: K, value: V) -> Option<V>;
}

impl<K, V> Dictionary<K, V> for HashMap<K, V> where K: Hash + Eq + Borrow<str> {
    fn to_list<'a>(&'a self) -> Vec<(&'a K, &'a V)> {
        self.iter().collect()
    }

    fn lookup<'a>(&'a self, key: &str) -> Option<&'a V> {
        self.get(key)
    }

    fn insert(&mut self, key: K, value: V) -> Option<V> {
        self.insert(key, value)
    }
}

impl<K, V> Dictionary<K, V> for BTreeMap<K, V> where K: Ord + Borrow<str> {
    fn to_list<'a>(&'a self) -> Vec<(&'a K, &'a V)> {
        self.iter().collect()
    }

    fn lookup<'a>(&'a self, key: &str) -> Option<&'a V> {
        self.get(key)
    }

    fn insert(&mut self, key: K, value: V) -> Option<V> {
        self.insert(key, value)
    }
}