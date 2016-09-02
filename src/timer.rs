use {Builder, wheel};
use worker::Worker;
use wheel::{Token, Wheel};
use futures::{Future, Poll};
use futures::task::{self, Task};
use std::time::Instant;

/// A facility for scheduling timeouts
#[derive(Clone)]
pub struct Timer {
    worker: Worker,
}

/// A `Future` that completes at the requested instance
pub struct Timeout {
    worker: Worker,
    when: Instant,
    handle: Option<(Task, Token)>,
}

pub fn build(builder: Builder) -> Timer {
    let wheel = Wheel::new(&builder);
    let worker = Worker::spawn(wheel, builder.get_channel_capacity());

    Timer { worker: worker }
}

impl Timer {
    /// Returns a future that completes once the given instant has been reached
    pub fn set_timeout(&self, when: Instant) -> Timeout {
        Timeout {
            worker: self.worker.clone(),
            when: when,
            handle: None,
        }
    }
}

impl Default for Timer {
    fn default() -> Timer {
        wheel().build()
    }
}

/*
 *
 * ===== Timeout =====
 *
 */

impl Timeout {
    pub fn is_expired(&self) -> bool {
        Instant::now() >= self.when
    }
}

impl Future for Timeout {
    type Item = ();
    type Error = ();

    fn poll(&mut self) -> Poll<(), ()> {
        trace!("Timeout::poll");

        if self.is_expired() {
            trace!("  --> expired; returning");
            return Poll::Ok(());
        }

        // The timeout has not expired, so perform any necessary operations
        // with the timer worker in order to get notified after the requested
        // instant.

        let handle = match self.handle {
            None => {
                // An wakeup request has not yet been sent to the timer.

                trace!("  --> no handle; parking");

                // Get the current task handle
                let task = task::park();

                match self.worker.set_timeout(self.when, task.clone()) {
                    Ok(token) => (task, token),
                    Err(task) => {
                        // The timer is overloaded, yield the current task
                        task.unpark();
                        return Poll::NotReady;
                    }
                }
            }
            Some((ref task, token)) => {
                if task.is_current() {
                    trace!("  --> handle current -- NotReady");

                    // Nothing more to do, the notify on timeout has already
                    // been registered
                    return Poll::NotReady;
                }

                trace!("  --> timeout moved -- notifying timer");

                let task = task::park();

                // The timeout has been moved to another task, in this case the
                // timer has to be notified
                match self.worker.move_timeout(token, self.when, task.clone()) {
                    Ok(_) => (task, token),
                    Err(task) => {
                        // Overloaded timer, yield hte current task
                        task.unpark();
                        return Poll::NotReady;
                    }
                }
            }
        };

        trace!("  --> tracking handle");

        // Moved out here to make the borrow checker happy
        self.handle = Some(handle);

        Poll::NotReady
    }
}

impl Drop for Timeout {
    fn drop(&mut self) {
        if let Some((_, token)) = self.handle {
            self.worker.cancel_timeout(token, self.when);
        }
    }
}
