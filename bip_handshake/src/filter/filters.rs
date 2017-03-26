use std::rc::Rc;
use std::sync::RwLock;

use filter::{HandshakeFilter};

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
                Some(_) => (),
                None    => { mut_filters.push(Box::new(filter)); }
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

#[cfg(test)]
pub mod test_filters {
    use std::net::SocketAddr;
    use std::any::Any;

    use message::protocol::Protocol;
    use filter::{HandshakeFilter, FilterDecision};

    #[derive(PartialEq, Eq)]
    pub struct BlockAddrFilter {
        addr: SocketAddr
    }

    impl BlockAddrFilter {
        pub fn new(addr: SocketAddr) -> BlockAddrFilter {
            BlockAddrFilter{ addr: addr }
        }
    }

    impl HandshakeFilter for BlockAddrFilter {
        fn as_any(&self) -> &Any {
            self
        }

        fn on_addr(&self, opt_addr: Option<&SocketAddr>) -> FilterDecision {
            match opt_addr {
                Some(in_addr) if in_addr == &self.addr => FilterDecision::Block,
                Some(_) => FilterDecision::Pass,
                None => FilterDecision::NeedData
            }
        }
    }

    #[derive(PartialEq, Eq)]
    pub struct BlockProtocolFilter {
        prot: Protocol
    }

    impl BlockProtocolFilter {
        pub fn new(prot: Protocol) -> BlockProtocolFilter {
            BlockProtocolFilter{ prot: prot }
        }
    }

    impl HandshakeFilter for BlockProtocolFilter {
        fn as_any(&self) -> &Any {
            self
        }

        fn on_prot(&self, opt_prot: Option<&Protocol>) -> FilterDecision {
            match opt_prot {
                Some(in_prot) if in_prot == &self.prot => FilterDecision::Block,
                Some(_) => FilterDecision::Pass,
                None => FilterDecision::NeedData
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::Filters;
    use super::test_filters::BlockAddrFilter;

    #[test]
    fn positive_add_filter() {
        let filters = Filters::new();

        let mut num_filters = 0;
        filters.access_filters(|filters| {
            num_filters += filters.len();
        });

        assert_eq!(0, num_filters);

        filters.add_filter(BlockAddrFilter::new("43.43.43.43:4343".parse().unwrap()));
        filters.access_filters(|filters| {
            num_filters += filters.len();
        });

        assert_eq!(1, num_filters);
    }

    #[test]
    fn positive_add_filter_already_present() {
        let filters = Filters::new();

        let mut num_filters = 0;
        filters.access_filters(|filters| {
            num_filters += filters.len();
        });

        assert_eq!(0, num_filters);

        filters.add_filter(BlockAddrFilter::new("43.43.43.43:4343".parse().unwrap()));
        filters.add_filter(BlockAddrFilter::new("43.43.43.43:4343".parse().unwrap()));
        filters.access_filters(|filters| {
            num_filters += filters.len();
        });

        assert_eq!(1, num_filters);
    }

    #[test]
    fn positive_remove_filter() {
        let filters = Filters::new();

        filters.add_filter(BlockAddrFilter::new("43.43.43.43:4343".parse().unwrap()));
        filters.remove_filter(BlockAddrFilter::new("43.43.43.43:4343".parse().unwrap()));

        let mut num_filters = 0;
        filters.access_filters(|filters| {
            num_filters += filters.len();
        });

        assert_eq!(0, num_filters);
    }

    #[test]
    fn positive_remove_filter_not_present() {
        let filters = Filters::new();

        filters.add_filter(BlockAddrFilter::new("43.43.43.43:4343".parse().unwrap()));
        filters.remove_filter(BlockAddrFilter::new("43.43.43.43:4342".parse().unwrap()));

        let mut num_filters = 0;
        filters.access_filters(|filters| {
            num_filters += filters.len();
        });

        assert_eq!(1, num_filters);
    }

    #[test]
    fn positive_remove_filter_multiple_present() {
        let filters = Filters::new();

        filters.add_filter(BlockAddrFilter::new("43.43.43.43:4343".parse().unwrap()));
        filters.add_filter(BlockAddrFilter::new("43.43.43.43:4344".parse().unwrap()));
        filters.remove_filter(BlockAddrFilter::new("43.43.43.43:4343".parse().unwrap()));

        let mut num_filters = 0;
        filters.access_filters(|filters| {
            num_filters += filters.len();
        });

        assert_eq!(1, num_filters);
    }

    #[test]
    fn positive_clear_filters_none_present() {
        let filters = Filters::new();

        filters.clear_filters();

        let mut num_filters = 0;
        filters.access_filters(|filters| {
            num_filters += filters.len();
        });

        assert_eq!(0, num_filters);
    }

    #[test]
    fn positive_clear_filters_one_present() {
        let filters = Filters::new();

        filters.add_filter(BlockAddrFilter::new("43.43.43.43:4343".parse().unwrap()));

        filters.clear_filters();

        let mut num_filters = 0;
        filters.access_filters(|filters| {
            num_filters += filters.len();
        });

        assert_eq!(0, num_filters);
    }

    #[test]
    fn positive_clear_filters_multiple_present() {
        let filters = Filters::new();

        filters.add_filter(BlockAddrFilter::new("43.43.43.43:4343".parse().unwrap()));
        filters.add_filter(BlockAddrFilter::new("43.43.43.43:4342".parse().unwrap()));
        filters.add_filter(BlockAddrFilter::new("43.43.43.43:4341".parse().unwrap()));

        filters.clear_filters();

        let mut num_filters = 0;
        filters.access_filters(|filters| {
            num_filters += filters.len();
        });

        assert_eq!(0, num_filters);
    }
}