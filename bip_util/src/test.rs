use chrono::{Duration, UTC, DateTime};

/// Allows us to time travel into the future.
pub fn travel_into_future(offset: Duration) -> DateTime<UTC> {
    UTC::now().checked_add(offset).unwrap()
}
    
/// Allows us to time travel into the past.
pub fn travel_into_past(offset: Duration) -> DateTime<UTC> {
    UTC::now().checked_sub(offset).unwrap()
}