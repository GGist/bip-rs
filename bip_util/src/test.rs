use std::mem::{self};

use time::{self, Duration, PreciseTime};

/// Allows us to time travel into the future.
pub fn travel_into_future(offset: Duration) -> PreciseTime {
    let offset_ns = offset.num_nanoseconds().unwrap() as u64;
    let desired_time = time::precise_time_ns() + offset_ns;
        
    unsafe{ mem::transmute(desired_time) }
}
    
/// Allows us to time travel into the past.
pub fn travel_into_past(offset: Duration) -> PreciseTime {
    let offset_ns = offset.num_nanoseconds().unwrap() as u64;
    let desired_time = time::precise_time_ns() - offset_ns;
        
    unsafe{ mem::transmute(desired_time) }
}