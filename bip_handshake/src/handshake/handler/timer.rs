use std::time::Duration;

use futures::Future;
use tokio_timer::{Timer, TimeoutError, Timeout};

#[derive(Clone)]
pub struct HandshakeTimer {
    timer:    Timer,
    duration: Duration
}

impl HandshakeTimer {
    pub fn new(timer: Timer, duration: Duration) -> HandshakeTimer {
        HandshakeTimer{ timer, duration }
    }

    pub fn timeout<F, E>(&self, future: F) -> Timeout<F>
        where F: Future<Error=E>, E: From<TimeoutError<F>> {
        self.timer.timeout(future, self.duration)
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::HandshakeTimer;

    use futures::future::{self, Future};
    use tokio_timer;
    
    #[test]
    fn positive_finish_before_timeout() {
        let timer = HandshakeTimer::new(tokio_timer::wheel().build(), Duration::from_millis(50));
        let result = timer.timeout(future::ok::<&'static str, ()>("Hello")).wait().unwrap();

        assert_eq!("Hello", result);
    }

    #[test]
    #[should_panic]
    fn negative_finish_after_timeout() {
        let timer = HandshakeTimer::new(tokio_timer::wheel().build(), Duration::from_millis(50));

        timer.timeout(future::empty::<(), ()>()).wait().unwrap();
    }
}