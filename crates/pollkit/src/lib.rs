//! Helpers for loops that poll instead of await: a [`Job`] that runs work in the
//! background and hands back its result on a later tick, an [`Every`] throttle for
//! "do this at most once per interval", and a [`Cooldown`] for "not again until then".

use std::fmt;
use std::sync::mpsc::{self, Receiver, TryRecvError};
use std::sync::{Arc, Mutex, OnceLock, PoisonError};
use std::time::{Duration, Instant};

static WAKE_HOOK: OnceLock<fn()> = OnceLock::new();

/// Registers a function that runs on the worker thread whenever a job finishes,
/// so the polling loop can wake up early instead of waiting for its next tick.
pub fn set_wake_hook(hook: fn()) {
    let _ = WAKE_HOOK.set(hook);
}

fn wake() {
    if let Some(hook) = WAKE_HOOK.get() {
        hook();
    }
}

#[derive(Default)]
struct CancelSlot(Mutex<Option<Box<dyn FnOnce() + Send>>>);

impl CancelSlot {
    fn set(&self, action: impl FnOnce() + Send + 'static) {
        *self.0.lock().unwrap_or_else(PoisonError::into_inner) = Some(Box::new(action));
    }

    fn fire(&self) {
        let action = self.0.lock().unwrap_or_else(PoisonError::into_inner).take();
        if let Some(action) = action {
            action();
        }
    }
}

/// The worker went away without producing a result: its thread panicked, failed to
/// spawn, or dropped its [`Done`] handle.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Aborted;

impl fmt::Display for Aborted {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("background job ended without a result")
    }
}

impl std::error::Error for Aborted {}

/// Completion handle for a [`Job`]. Move it into whatever runs the work and call
/// [`Done::send`] exactly once.
pub struct Done<T> {
    sender: mpsc::Sender<T>,
    cancel: Arc<CancelSlot>,
}

impl<T> Done<T> {
    /// Delivers the result and wakes the polling loop.
    pub fn send(self, value: T) {
        if self.sender.send(value).is_ok() {
            wake();
        }
    }

    /// Sets what [`Job::cancel`] does. Calling it again replaces the previous action,
    /// which lets a worker keep the action pointed at its current inner operation.
    pub fn on_cancel(&self, action: impl FnOnce() + Send + 'static) {
        self.cancel.set(action);
    }
}

/// A unit of background work polled from a synchronous loop.
///
/// A job is either idle or running. [`Job::poll`] returns the result once and the job
/// goes back to idle. Dropping or replacing a running job runs its cancel action.
pub struct Job<T> {
    receiver: Option<Receiver<T>>,
    cancel: Arc<CancelSlot>,
}

impl<T> Default for Job<T> {
    fn default() -> Self {
        Self::idle()
    }
}

impl<T> Job<T> {
    pub fn idle() -> Self {
        Self {
            receiver: None,
            cancel: Arc::default(),
        }
    }

    /// A running job plus the handle that completes it. Use this when the work is
    /// driven by something other than a plain thread, such as an async runtime.
    pub fn pending() -> (Self, Done<T>) {
        let (sender, receiver) = mpsc::channel();
        let cancel = Arc::new(CancelSlot::default());
        let job = Self {
            receiver: Some(receiver),
            cancel: Arc::clone(&cancel),
        };
        (job, Done { sender, cancel })
    }

    pub fn is_running(&self) -> bool {
        self.receiver.is_some()
    }

    /// Takes the result if the job has finished. Returns `None` while idle or running.
    pub fn poll(&mut self) -> Option<Result<T, Aborted>> {
        let outcome = match self.receiver.as_ref()?.try_recv() {
            Ok(value) => Ok(value),
            Err(TryRecvError::Empty) => return None,
            Err(TryRecvError::Disconnected) => Err(Aborted),
        };
        self.finish();
        Some(outcome)
    }

    /// Like [`Job::poll`], treating an aborted job the same as one still running.
    pub fn poll_ok(&mut self) -> Option<T> {
        self.poll()?.ok()
    }

    /// Runs the cancel action (if the worker registered one) and returns to idle.
    pub fn cancel(&mut self) {
        if self.receiver.take().is_some() {
            self.cancel.fire();
            self.cancel = Arc::default();
        }
    }

    fn finish(&mut self) {
        self.receiver = None;
        self.cancel = Arc::default();
    }
}

impl<T: Send + 'static> Job<T> {
    /// Runs `work` on a new thread.
    pub fn spawn(work: impl FnOnce() -> T + Send + 'static) -> Self {
        Self::start(std::thread::Builder::new(), work)
    }

    /// Runs `work` on a new thread with the given name.
    pub fn spawn_named(name: &str, work: impl FnOnce() -> T + Send + 'static) -> Self {
        Self::start(std::thread::Builder::new().name(name.to_string()), work)
    }

    fn start(builder: std::thread::Builder, work: impl FnOnce() -> T + Send + 'static) -> Self {
        let (job, done) = Self::pending();
        if let Err(error) = builder.spawn(move || done.send(work())) {
            log::warn!("Failed to spawn job thread: {error}");
        }
        job
    }
}

impl<T> Drop for Job<T> {
    fn drop(&mut self) {
        self.cancel();
    }
}

/// Rate limits a recurring action: [`Every::due`] is true at most once per interval.
#[derive(Debug, Clone, Copy)]
pub struct Every {
    interval: Duration,
    last: Option<Instant>,
}

impl Every {
    /// First due one interval from now.
    pub fn new(interval: Duration) -> Self {
        Self {
            interval,
            last: Some(Instant::now()),
        }
    }

    pub fn secs(secs: u64) -> Self {
        Self::new(Duration::from_secs(secs))
    }

    pub fn millis(millis: u64) -> Self {
        Self::new(Duration::from_millis(millis))
    }

    /// True when an interval has passed since the last time this returned true.
    /// Put it last in a condition chain so it only ticks when the action runs.
    pub fn due(&mut self, now: Instant) -> bool {
        if self
            .last
            .is_some_and(|last| now.saturating_duration_since(last) < self.interval)
        {
            return false;
        }
        self.last = Some(now);
        true
    }

    pub fn due_now(&mut self) -> bool {
        self.due(Instant::now())
    }

    /// Makes the next [`Every::due`] call succeed immediately.
    pub fn expire(&mut self) {
        self.last = None;
    }

    /// Counts `now` as a tick, as if the action had just run.
    pub fn restart(&mut self, now: Instant) {
        self.last = Some(now);
    }
}

/// A one-shot wait: [`Cooldown::start`] it after something happens and check
/// [`Cooldown::is_ready`] before doing it again. Starts out ready.
#[derive(Debug, Clone, Copy, Default)]
pub struct Cooldown {
    until: Option<Instant>,
}

impl Cooldown {
    pub const fn ready() -> Self {
        Self { until: None }
    }

    pub fn start(&mut self, now: Instant, duration: Duration) {
        self.until = Some(now + duration);
    }

    pub fn start_now(&mut self, duration: Duration) {
        self.start(Instant::now(), duration);
    }

    pub fn is_ready(&self, now: Instant) -> bool {
        self.until.is_none_or(|until| now >= until)
    }

    pub fn is_ready_now(&self) -> bool {
        self.is_ready(Instant::now())
    }

    pub fn clear(&mut self) {
        self.until = None;
    }
}
