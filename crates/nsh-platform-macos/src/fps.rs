//! The performance monitor a surface measures with, on macOS.
//!
//! The panel that used to draw the readings here is gone: an app draws its
//! own with `nsh/perf` (`snapshot()`), one overlay for every platform. What
//! stays is the monitor — it samples the display's rate and the process'
//! CPU, memory and GPU, which `nsh/perf` hands to the app.

use std::{
    cell::{Cell, RefCell},
    rc::Rc,
    sync::Arc,
};

use nsh_engine::perf::{PerfMonitor, ProcessSampler};

use crate::app::AppView;

/// A monitor a surface measures with, kept alive while the app shows
/// readings. Dropping it stops the sampling.
pub(crate) struct Monitor {
    monitor: Arc<PerfMonitor>,
    process: RefCell<ProcessSampler>,
    gpu: RefCell<Option<crate::gpu::GpuProbe>>,
    updates: Cell<u64>,
}

impl Monitor {
    /// Starts measuring `view`'s surface.
    pub(crate) fn start(view: &AppView) -> Rc<Monitor> {
        let monitor = Arc::new(PerfMonitor::new());
        let started = Rc::new(Monitor {
            monitor: monitor.clone(),
            process: RefCell::new(ProcessSampler::new()),
            gpu: RefCell::new(crate::gpu::GpuProbe::new()),
            updates: Cell::new(0),
        });
        view.surface().set_perf_monitor(Some(monitor));
        started.sample();
        started
    }

    /// The monitor `nsh/perf` reads.
    pub(crate) fn monitor(&self) -> Arc<PerfMonitor> {
        self.monitor.clone()
    }

    /// Samples the process and the GPU, which the monitor cannot read itself
    /// (they are the platform's).
    pub(crate) fn sample(&self) {
        let mut process = self.process.borrow_mut().sample();
        process.gpu = self.gpu.borrow_mut().as_mut().and_then(|g| g.sample());
        self.monitor.set_process(process);
        self.updates.set(self.updates.get() + 1);
    }
}
