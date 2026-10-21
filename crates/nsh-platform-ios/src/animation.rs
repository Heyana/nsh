//! Runs the native animation graph on the main thread, driven by a
//! `CADisplayLink` that only runs while something animates.

use nsh_animation::{AnimationOp, AnimationValue, Finished, Graph, NodeId};
use objc2::{rc::Retained, sel};
use objc2_foundation::{NSRunLoop, NSRunLoopCommonModes};
use objc2_quartz_core::CADisplayLink;

use crate::target::EventTarget;

pub(crate) struct Animator {
    graph: Graph,
    finished: Vec<Finished>,
    link: Retained<CADisplayLink>,
    running: bool,
    /// Run the link even with nothing animating (a monitor is installed).
    always: bool,
}

/// The rates the display link asks for: the screen's highest (120 Hz on
/// ProMotion), at least 80 there. Without it the link settles on 60 Hz.
fn frame_rate_range() -> objc2_quartz_core::CAFrameRateRange {
    let mtm = objc2::MainThreadMarker::new().expect("the display link is main-thread only");
    let max = objc2_ui_kit::UIScreen::mainScreen(mtm)
        .maximumFramesPerSecond()
        .max(1) as f32;
    objc2_quartz_core::CAFrameRateRange {
        minimum: max.min(80.0),
        maximum: max,
        preferred: max,
    }
}

impl Animator {
    pub(crate) fn new(target: &EventTarget) -> Self {
        // SAFETY: `tick:` is implemented by `EventTarget`, which outlives the
        // link (the link is invalidated in `Drop`).
        let link = unsafe { CADisplayLink::displayLinkWithTarget_selector(target, sel!(tick:)) };
        link.setPreferredFrameRateRange(frame_rate_range());
        link.setPaused(true);
        // SAFETY: the main run loop and standard modes. Common modes keep
        // animations running while a scroll view is tracking.
        unsafe { link.addToRunLoop_forMode(&NSRunLoop::mainRunLoop(), NSRunLoopCommonModes) };
        Self {
            graph: Graph::default(),
            finished: Vec::new(),
            link,
            running: false,
            always: false,
        }
    }

    pub(crate) fn apply(&mut self, op: AnimationOp) {
        self.graph.apply(op, &mut self.finished);
        self.update_link();
    }

    /// Sets a value from a gesture or a scroll position.
    pub(crate) fn set(&mut self, node: NodeId, value: f32) {
        self.graph.set(node, value, &mut self.finished);
        self.update_link();
    }

    pub(crate) fn value(&self, node: NodeId) -> f32 {
        self.graph.value(node).unwrap_or(0.0)
    }

    pub(crate) fn eval(&self, node: NodeId) -> AnimationValue {
        self.graph.eval(node)
    }

    /// Advances to the display link's target time. Returns true if values
    /// changed.
    pub(crate) fn tick(&mut self) -> bool {
        let changed = self
            .graph
            .tick(self.link.targetTimestamp(), &mut self.finished);
        self.update_link();
        changed
    }

    pub(crate) fn take_finished(&mut self) -> Vec<Finished> {
        std::mem::take(&mut self.finished)
    }

    fn update_link(&mut self) {
        // A performance monitor needs every refresh, not only the animating
        // ones: it counts frames from the gaps between them.
        let wanted = self.graph.is_animating() || self.always;
        if wanted != self.running {
            self.running = wanted;
            self.link.setPaused(!wanted);
        }
    }

    /// The link's target time (`Instant`, what the frame is drawn for) and
    /// the period between refreshes: the monitor closes a frame at each.
    pub(crate) fn frame_timing(&self) -> (nsh_core::time::Instant, std::time::Duration) {
        let period = self.link.duration();
        // The display link counts from boot in `CACurrentMediaTime` seconds;
        // the engine's `Instant` only needs differences, so shift both.
        let target = objc2_quartz_core::CACurrentMediaTime()
            + (self.link.targetTimestamp() - objc2_quartz_core::CACurrentMediaTime());
        let now = nsh_core::time::Instant::now();
        let _ = target;
        (
            now,
            std::time::Duration::from_secs_f64(period.max(0.0)),
        )
    }

    /// Keeps the display link running whether or not anything animates: a
    /// performance monitor measures every frame.
    pub(crate) fn set_always_running(&mut self, always: bool) {
        self.always = always;
        self.update_link();
    }
}

/// A paused display link calling `frame:` on `target`, unpaused for one
/// frame when JS calls `requestAnimationFrame`.
pub(crate) fn frame_link(target: &EventTarget) -> Retained<CADisplayLink> {
    // SAFETY: `frame:` is implemented by `EventTarget`, which outlives the
    // link (the mounter invalidates it when dropped).
    let link = unsafe { CADisplayLink::displayLinkWithTarget_selector(target, sel!(frame:)) };
    link.setPreferredFrameRateRange(frame_rate_range());
    link.setPaused(true);
    // SAFETY: the main run loop and standard modes.
    unsafe { link.addToRunLoop_forMode(&NSRunLoop::mainRunLoop(), NSRunLoopCommonModes) };
    link
}

impl Drop for Animator {
    fn drop(&mut self) {
        self.link.invalidate();
    }
}
