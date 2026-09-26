use crate::{MetricSelection, PlatformError, SystemSample};

/// Snapshot methods block, are callable from background threads, retain no caller data,
/// return unavailable capabilities as errors, and are not reentrant.
pub trait SystemMetrics {
    /// Samples selected counters; an error means the OS did not provide them.
    fn sample(&self, selection: MetricSelection) -> Result<SystemSample, PlatformError>;
    /// Trims this process's working set, or returns an OS error.
    fn trim_working_set(&self) -> Result<(), PlatformError>;
}
