//! 

use std::iter;

pub struct TierList<'a> {
    tiers: Vec<Vec<&'a str>>
}

impl<'a> TierList<'a> {
    pub fn new(tier_list: Vec<Vec<&'a str>>) -> TierList<'a> {
        TierList{ tiers: tier_list }
    }
    
    pub fn iter<'b>(&'b mut self) -> TierIter<'a, 'b> {
        TierIter{ tiers: &mut self.tiers[..], index: 0 }
    }
}

pub struct TierIter<'a: 'b, 'b> {
    tiers: &'b mut [Vec<&'a str>],
    index: usize
}

impl<'a: 'b, 'b: 'c, 'c> Iterator for TierIter<'a, 'b> {
    type Item = Tier<'a, 'c>;
    
    fn next(&mut self) -> Option<<Self as Iterator>::Item> {
        if self.index >= self.tiers.len() {
            None
        } else {
            self.index += 1;
            Some(Tier{ tier: &mut self.tiers[self.index - 1][..] })
        }
    }
}

pub struct Tier<'a: 'b, 'b> {
    tier: &'b mut [&'a str]
}

impl<'a: 'b, 'b> Tier<'a, 'b> {
    pub fn check_tier<F>(&mut self, is_active: F) -> bool 
        where F: Fn(&str) -> bool {
        let mut active_index = -1;
        let mut active_url = "";
    
        for (index, url) in self.tier.iter().enumerate() {
            if is_active(url) {
                active_index = index;
                active_url = url;
                
                break;
            }
        }
        
        if active_index != -1 {
            for i in iter::range_step_inclusive(active_index - 1, 0, 1) {
                let move_value = self.tier[i];
                
                self.tier[i + 1] = move_value;
            }
            self.tier[0] = active_url;
        
            true
        } else {
            false
        }
    }
}