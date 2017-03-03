use std::rc::Rc;
use std::sync::RwLock;

use filter::{HandshakeFilter, FilterDecision};

#[derive(Clone)]
pub struct Filters {
    filters: Rc<RwLock<Vec<Box<HandshakeFilter>>>>
}

impl Filters {
    pub fn new() -> Filters {
        Filters{ filters: Rc::new(RwLock::new(Vec::new())) }
    }

    pub fn add_filter<F>(&self, filter: F)
        where F: HandshakeFilter + PartialEq + Eq + 'static {
        self.write_filters(|mut_filters| {
            let opt_found = check_index(&mut_filters[..], &filter);

            match opt_found {
                Some(index) => (),
                None        => { mut_filters.push(Box::new(filter)); }
            }
        });
    }

    pub fn remove_filter<F>(&self, filter: F)
        where F: HandshakeFilter + PartialEq + Eq + 'static {
        self.write_filters(|mut_filters| {
            let opt_found = check_index(&mut_filters[..], &filter);

            match opt_found {
                Some(index) => { mut_filters.swap_remove(index); },
                None        => ()
            }
        });
    }

    pub fn access_filters<B>(&self, block: B)
        where B: FnOnce(&[Box<HandshakeFilter>]) {
        self.read_filters(|ref_filters| {
            block(ref_filters)
        })
    }

    pub fn clear_filters(&self) {
        self.write_filters(|mut_filters| {
            mut_filters.clear();
        });
    }

    fn read_filters<B, R>(&self, block: B) -> R
        where B: FnOnce(&[Box<HandshakeFilter>]) -> R {
        let ref_filters = self.filters.as_ref().read()
            .expect("bip_handshake: Poisoned Read Lock In Filters");
        
        block(&ref_filters)
    }

    fn write_filters<B, R>(&self, block: B) -> R
        where B: FnOnce(&mut Vec<Box<HandshakeFilter>>) -> R {
        let mut mut_filters = self.filters.as_ref().write()
            .expect("bip_handshake: Poisoned Write Lock In Filters");

        block(&mut mut_filters)
    }
}

fn check_index<F>(ref_filters: &[Box<HandshakeFilter>], filter: &F) -> Option<usize>
    where F: HandshakeFilter + PartialEq + Eq + 'static {
    for (index, ref_filter) in ref_filters.into_iter().enumerate() {
        let opt_match = ref_filter.as_any().downcast_ref::<F>()
            .map(|downcast_filter| downcast_filter == filter);

        match opt_match {
            Some(true)         => { return Some(index) },
            Some(false) | None => ()
        }
    }

    None
}