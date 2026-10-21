//! The performance monitor a surface measures with, on iOS.
//!
//! The panel that used to draw the readings here is gone: an app draws its
//! own with `nsh/perf` (`snapshot()`), one overlay for every platform. What
//! stays is the monitor — it samples the display's rate and the process'
//! CPU and memory, which `nsh/perf` hands to the app.

use std::{cell::RefCell, sync::Arc};

use nsh_engine::perf::{PerfMonitor, ProcessSampler};

/// A monitor a surface measures with, kept alive while the app shows
/// readings. Dropping it stops the sampling and the frame timing.
pub(crate) struct Monitor {
    monitor: Arc<PerfMonitor>,
    process: RefCell<ProcessSampler>,
    updates: std::cell::Cell<u64>,
}

impl Monitor {
    /// Starts measuring `surface`'s frames, with `view` as its surface.
    pub(crate) fn start(view: &crate::surface::RootView) -> Arc<Monitor> {
        let monitor = Arc::new(PerfMonitor::new());
        // The surface's frames are timed from now on, and the engine's
        // commits are reported to the same monitor.
        view.set_perf_monitor(Some(monitor.clone()));
        let started = Arc::new(Monitor {
            monitor: monitor.clone(),
            process: RefCell::new(ProcessSampler::new()),
            updates: std::cell::Cell::new(0),
        });
        started.sample();
        started
    }

    /// The monitor `nsh/perf` reads.
    pub(crate) fn monitor(&self) -> Arc<PerfMonitor> {
        self.monitor.clone()
    }

    /// Samples the process, which the monitor cannot read itself (it is the
    /// platform's).
    pub(crate) fn sample(&self) {
        self.monitor.set_process(self.process.borrow_mut().sample());
        self.updates.set(self.updates.get() + 1);
    }
}
