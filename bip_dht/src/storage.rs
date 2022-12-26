use std::collections::HashMap;
use std::collections::hash_map::Entry;
use std::net::SocketAddr;

use bip_util::bt::InfoHash;
use chrono::{UTC, DateTime, Duration};

const MAX_ITEMS_STORED: usize = 500;

/// Manages storage and expiration of contact information for a number of InfoHashs.
pub struct AnnounceStorage {
    storage: HashMap<InfoHash, Vec<AnnounceItem>>,
    expires: Vec<ItemExpiration>,
}

impl AnnounceStorage {
    /// Create a new AnnounceStorage object.
    pub fn new() -> AnnounceStorage {
        AnnounceStorage {
            storage: HashMap::new(),
            expires: Vec::new(),
        }
    }

    /// Returns true if the item was added/it's existing expiration updated, false otherwise.
    pub fn add_item(&mut self, info_hash: InfoHash, address: SocketAddr) -> bool {
        self.add(info_hash, address, UTC::now())
    }

    fn add(&mut self, info_hash: InfoHash, address: SocketAddr, curr_time: DateTime<UTC>) -> bool {
        // Clear out any old contacts that we have stored
        self.remove_expired_items(curr_time);
        let item = AnnounceItem::new(info_hash, address);
        let item_expiration = item.expiration();

        // Check if we already have the item and want to update it's expiration
        match self.insert_contact(item) {
            Some(true) => {
                self.expires.retain(|i| i != &item_expiration);
                self.expires.push(item_expiration);

                true
            }
            Some(false) => {
                self.expires.push(item_expiration);

                true
            }
            None => false,
        }
    }

    /// Invoke the closure once for each contact for the given InfoHash.
    pub fn find_items<F>(&mut self, info_hash: &InfoHash, item_func: F)
        where F: FnMut(SocketAddr)
    {
        self.find(info_hash, item_func, UTC::now())
    }

    fn find<F>(&mut self, info_hash: &InfoHash, mut item_func: F, curr_time: DateTime<UTC>)
        where F: FnMut(SocketAddr)
    {
        // Clear out any old contacts that we have stored
        self.remove_expired_items(curr_time);

        if let Some(items) = self.storage.get(info_hash) {
            for item in items {
                item_func(item.address());
            }
        }
    }

    /// Returns None if the contact could not be inserted, else, returns Some(true) if the contact was already
    /// in the table (and was replaced by the new entry) or Some(false) if the contact was not already in the
    /// table but was inserted.
    fn insert_contact(&mut self, item: AnnounceItem) -> Option<bool> {
        let item_info_hash = item.info_hash();

        // Check if the contact is already in our list
        let already_in_list = if let Some(items) = self.storage.get_mut(&item_info_hash) {
            items.iter().any(|a| a == &item)
        } else {
            false
        };

        // Check if we need to insert it into the list and if we have room
        match (already_in_list, self.expires.len() < MAX_ITEMS_STORED) {
            (false, true) => {
                // Place it into the appropriate list
                match self.storage.entry(item_info_hash) {
                    Entry::Occupied(mut occ) => occ.get_mut().push(item),
                    Entry::Vacant(vac) => {
                        vac.insert(vec![item]);
                    }
                };

                Some(false)
            }
            (false, false) => None,
            (true, false) => Some(true),
            (true, true) => Some(true),
        }
    }

    /// Prunes all expired items from the internal list.
    fn remove_expired_items(&mut self, curr_time: DateTime<UTC>) {
        let num_expired_items = self.expires.iter().take_while(|i| i.is_expired(curr_time)).count();

        // Remove the numbers of expired elements from the head of the list
        for item_expiration in self.expires.drain(0..num_expired_items) {
            let info_hash = item_expiration.info_hash();

            // Get a mutable reference to the list of contacts and remove all contacts that
            // are associated with the expiration (should only be one such contact).
            let remove_info_hash = if let Some(items) = self.storage.get_mut(&info_hash) {
                items.retain(|a| a.expiration() != item_expiration);

                items.is_empty()
            } else {
                false
            };

            // If we drained the list of contacts completely, remove the info hash entry
            if remove_info_hash {
                self.storage.remove(&info_hash);
            }
        }
    }
}

// ----------------------------------------------------------------------------//

#[derive(Debug, Clone, PartialEq, Eq)]
struct AnnounceItem {
    expiration: ItemExpiration,
}

impl AnnounceItem {
    pub fn new(info_hash: InfoHash, address: SocketAddr) -> AnnounceItem {
        AnnounceItem { expiration: ItemExpiration::new(info_hash, address) }
    }

    pub fn expiration(&self) -> ItemExpiration {
        self.expiration.clone()
    }

    pub fn address(&self) -> SocketAddr {
        self.expiration.address()
    }

    pub fn info_hash(&self) -> InfoHash {
        self.expiration.info_hash()
    }
}

// ----------------------------------------------------------------------------//

const EXPIRATION_TIME_HOURS: i64 = 24;

#[derive(Debug, Clone)]
struct ItemExpiration {
    address: SocketAddr,
    inserted: DateTime<UTC>,
    info_hash: InfoHash,
}

impl ItemExpiration {
    pub fn new(info_hash: InfoHash, address: SocketAddr) -> ItemExpiration {
        ItemExpiration {
            address,
            inserted: UTC::now(),
            info_hash,
        }
    }

    pub fn is_expired(&self, now: DateTime<UTC>) -> bool {
        now - self.inserted >= Duration::hours(EXPIRATION_TIME_HOURS)
    }

    pub fn info_hash(&self) -> InfoHash {
        self.info_hash
    }

    pub fn address(&self) -> SocketAddr {
        self.address
    }
}

impl PartialEq for ItemExpiration {
    fn eq(&self, other: &ItemExpiration) -> bool {
        self.address() == other.address() && self.info_hash() == other.info_hash()
    }
}

impl Eq for ItemExpiration {}

#[cfg(test)]
mod tests {
    use bip_util::bt;
    use bip_util::test as bip_test;

    use chrono::Duration;
    use crate::storage::{self, AnnounceStorage};

    #[test]
    fn positive_add_and_retrieve_contact() {
        let mut announce_store = AnnounceStorage::new();
        let info_hash = [0u8; bt::INFO_HASH_LEN].into();
        let sock_addr = bip_test::dummy_socket_addr_v4();

        assert!(announce_store.add_item(info_hash, sock_addr));

        let mut items = Vec::new();
        announce_store.find_items(&info_hash, |a| items.push(a));
        assert_eq!(items.len(), 1);

        assert_eq!(items[0], sock_addr);
    }

    #[test]
    fn positive_add_and_retrieve_contacts() {
        let mut announce_store = AnnounceStorage::new();
        let info_hash = [0u8; bt::INFO_HASH_LEN].into();
        let sock_addrs = bip_test::dummy_block_socket_addrs(storage::MAX_ITEMS_STORED as u16);

        for sock_addr in sock_addrs.iter() {
            assert!(announce_store.add_item(info_hash, *sock_addr));
        }

        let mut items = Vec::new();
        announce_store.find_items(&info_hash, |a| items.push(a));
        assert_eq!(items.len(), storage::MAX_ITEMS_STORED);

        for item in items.iter() {
            assert!(sock_addrs.iter().any(|s| s == item));
        }
    }

    #[test]
    fn positive_renew_contacts() {
        let mut announce_store = AnnounceStorage::new();
        let info_hash = [0u8; bt::INFO_HASH_LEN].into();
        let sock_addrs = bip_test::dummy_block_socket_addrs((storage::MAX_ITEMS_STORED + 1) as u16);

        for sock_addr in sock_addrs.iter().take(storage::MAX_ITEMS_STORED) {
            assert!(announce_store.add_item(info_hash, *sock_addr));
        }

        // Try to add a new item
        let other_info_hash = [1u8; bt::INFO_HASH_LEN].into();

        // Returns false because it wasnt added
        assert!(!announce_store.add_item(other_info_hash, sock_addrs[sock_addrs.len() - 1]));
        // Closure not invoked because it wasnt added
        let mut times_invoked = 0;
        announce_store.find_items(&other_info_hash, |_| times_invoked += 1);
        assert_eq!(times_invoked, 0);

        // Try to add all of the initial nodes again (renew)
        for sock_addr in sock_addrs.iter().take(storage::MAX_ITEMS_STORED) {
            assert!(announce_store.add_item(info_hash, *sock_addr));
        }
    }

    #[test]
    fn positive_full_storage_expire_one_infohash() {
        let mut announce_store = AnnounceStorage::new();
        let info_hash = [0u8; bt::INFO_HASH_LEN].into();
        let sock_addrs = bip_test::dummy_block_socket_addrs((storage::MAX_ITEMS_STORED + 1) as u16);

        // Fill up the announce storage completely
        for sock_addr in sock_addrs.iter().take(storage::MAX_ITEMS_STORED) {
            assert!(announce_store.add_item(info_hash, *sock_addr));
        }

        // Try to add a new item into the storage (under a different info hash)
        let other_info_hash = [1u8; bt::INFO_HASH_LEN].into();

        // Returned false because it wasnt added
        assert!(!announce_store.add_item(other_info_hash, sock_addrs[sock_addrs.len() - 1]));
        // Closure not invoked because it wasnt added
        let mut times_invoked = 0;
        announce_store.find_items(&other_info_hash, |_| times_invoked += 1);
        assert_eq!(times_invoked, 0);

        // Try to add a new item into the storage mocking the current time
        let mock_current_time =
            bip_test::travel_into_future(Duration::hours(storage::EXPIRATION_TIME_HOURS));
        assert!(announce_store.add(other_info_hash,
                                   sock_addrs[sock_addrs.len() - 1],
                                   mock_current_time));
        // Closure invoked because it was added
        announce_store.find_items(&other_info_hash, |_| times_invoked += 1);
        assert_eq!(times_invoked, 1);
    }

    #[test]
    fn positive_full_storage_expire_two_infohash() {
        let mut announce_store = AnnounceStorage::new();
        let info_hash_one = [0u8; bt::INFO_HASH_LEN].into();
        let info_hash_two = [1u8; bt::INFO_HASH_LEN].into();
        let sock_addrs = bip_test::dummy_block_socket_addrs((storage::MAX_ITEMS_STORED + 1) as u16);

        // Fill up first info hash
        let num_contacts_first = storage::MAX_ITEMS_STORED / 2;
        for sock_addr in sock_addrs.iter().take(num_contacts_first) {
            assert!(announce_store.add_item(info_hash_one, *sock_addr));
        }

        // Fill up second info hash
        let num_contacts_second = storage::MAX_ITEMS_STORED - num_contacts_first;
        for sock_addr in sock_addrs.iter().skip(num_contacts_first).take(num_contacts_second) {
            assert!(announce_store.add_item(info_hash_two, *sock_addr));
        }

        // Try to add a third info hash with a contact
        let info_hash_three = [2u8; bt::INFO_HASH_LEN].into();
        assert!(!announce_store.add_item(info_hash_three, sock_addrs[sock_addrs.len() - 1]));
        // Closure not invoked because it was not added
        let mut times_invoked = 0;
        announce_store.find_items(&info_hash_three, |_| times_invoked += 1);
        assert_eq!(times_invoked, 0);

        // Try to add a new item into the storage mocking the current time
        let mock_current_time =
            bip_test::travel_into_future(Duration::hours(storage::EXPIRATION_TIME_HOURS));
        assert!(announce_store.add(info_hash_three,
                                   sock_addrs[sock_addrs.len() - 1],
                                   mock_current_time));
        // Closure invoked because it was added
        announce_store.find_items(&info_hash_three, |_| times_invoked += 1);
        assert_eq!(times_invoked, 1);
    }
}
