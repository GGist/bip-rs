use std::collections::{HashMap};
use std::collections::hash_map::{Entry};
use std::sync::{RwLock, Mutex};

use bip_util::bt::{InfoHash};

/// A concurrent data structure that maps an info hash to many values V.
///
/// Concurrency is optimized for minimal writes to the mapping of info hashes
/// but many writes to the values mapped to each individual info hash.
#[derive(Debug)]
pub struct InfoHashMap<V> where V: 'static {
    map: RwLock<HashMap<InfoHash, Mutex<Vec<V>>>>
}

impl<V> InfoHashMap<V> {
    /// Create a new InfoHashMap.
    pub fn new() -> InfoHashMap<V> {
        InfoHashMap{ map: RwLock::new(HashMap::new()) }
    }
    
    /// Insert a value into the list of values associated with the given info hash.
    pub fn insert(&self, hash: InfoHash, value: V) {
        match self.map.write().unwrap().entry(hash) {
            Entry::Occupied(occ) => occ.get().lock().unwrap().push(value),
            Entry::Vacant(vac)   => { vac.insert(Mutex::new(vec![value])); },
        }
    }
    
    /// Checks if the given info hash has one or more values associated with it.
    ///
    /// Since this method also does some housekeeping, it should only be called by the main worker thread,
    /// other worker threads may want to operate under the assumption that the infohash has values.
    pub fn has_values(&self, hash: &InfoHash) -> bool {
        // Optimized for worst case scenario where we are checking 
        
        // Check if info hash has a mapping with a read lock.
        {
            let map_read = self.map.read().unwrap();
            
            match map_read.get(hash) {
                Some(mutex) => {
                    if !mutex.lock().unwrap().is_empty() {
                        return true
                    }
                },
                None => return false
            };
        }
        
        // Check if we can remove the info hash mapping after getting a write lock.
        {
            let mut map_write = self.map.write().unwrap();
            
            let should_remove = match map_write.get(hash) {
                Some(list_mutex) => {
                    let list = list_mutex.lock().unwrap();
                    
                    list.is_empty()
                },
                None => return false
            };
            
            if should_remove {
                map_write.remove(hash);
                
                false
            } else {
                true
            }
        }
    }
    
    /// Run a function against all values at the given info hash.
    ///
    /// If the function returns false, the value V passed in will be pruned.
    pub fn retain<F>(&self, hash: &InfoHash, mut f: F) where F: FnMut(&V) -> bool {
        let read_guard = self.map.read().unwrap();
        
        let mut mutex_guard = if let Some(mutex) = read_guard.get(hash) {
            mutex.lock().unwrap()
        } else {
            return
        };
        
        mutex_guard.retain(|value| {
            f(value)
        });
    }
}