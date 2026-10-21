//! The performance monitor's HUD for iOS ([`crate::Surface::set_fps_monitor`]):
//! a panel at the top right of a surface that shows what its frames cost
//! ([`nsh_engine::perf`]), redrawn twice a second.
//!
//! The readings are one per line, labelled ([`readings`]): a frame's cost,
//! its budget at the display's rate, and what share of frames went over it.
//! The distributions, commit counts and engine-side splits stay in the log
//! line (`--trace`), where they are read with the profiler.
//!
//! The panel lets touches through ([`PassView`]): it never intercepts input,
//! so a demo behaves the same with and without it.

use std::{
    cell::RefCell,
    rc::{Rc, Weak},
    sync::Arc,
    time::Duration,
};

use nsh_engine::perf::{PerfMonitor, PerfSnapshot, ProcessSampler};
use objc2::{DefinedClass, MainThreadMarker, MainThreadOnly, define_class, msg_send, rc::Retained};
use objc2_core_foundation::{CGFloat, CGPoint, CGRect, CGSize};
use objc2_core_graphics::CGContext;
use objc2_foundation::{NSObject, NSObjectProtocol, NSString};
use objc2_ui_kit::{UIView, UIViewController};
use rustc_hash::FxHashMap;

use crate::surface::RootView;

/// How often the readings are redrawn.
const INTERVAL_MS: u64 = 500;
/// The panel's width, its distance from the view's edges, and its padding
/// (points).
const WIDTH: CGFloat = 236.0;
const MARGIN: CGFloat = 10.0;
const PADDING: CGFloat = 8.0;
const GRAPH_HEIGHT: CGFloat = 36.0;

thread_local! {
    /// The HUDs of this (main) thread by key: the timer finds its HUD by
    /// key, as dispatched closures must be `Send`.
    static HUDS: RefCell<FxHashMap<u64, Weak<Hud>>> = RefCell::default();
}

static NEXT_KEY: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);

define_class!(
    /// The panel: a view that draws the readings and never takes a touch.
    #[unsafe(super(UIView))]
    #[name = "NSHPerfPanel"]
    #[ivars = ()]
    pub(crate) struct Panel;

    impl Panel {
        #[unsafe(method(hitTest:withEvent:))]
        fn hit_test(&self, _point: CGPoint, _event: *mut objc2_ui_kit::UIEvent) -> *mut UIView {
            // Touches fall through to whatever is under the panel.
            std::ptr::null_mut()
        }
    }
);

define_class!(
    /// Draws the frame trace: the last frames' cost against the budget.
    #[unsafe(super(UIView))]
    #[name = "NSHPerfTrace"]
    #[ivars = RefCell<Trace>]
    struct TraceView;

    impl TraceView {
        #[unsafe(method(drawRect:))]
        fn draw_rect(&self, rect: CGRect) {
            let Some(cg) = objc2_ui_kit::UIGraphicsGetCurrentContext() else {
                return;
            };
            // The context is UIKit's, which Core Graphics also sees.
            let trace = self.ivars().borrow();
            draw_trace(&cg, rect, &trace);
        }
    }
);

impl TraceView {
    fn new(mtm: MainThreadMarker) -> Retained<Self> {
        let this = mtm.alloc::<Self>().set_ivars(RefCell::new(Trace::default()));
        unsafe { msg_send![super(this), initWithFrame: CGRect::ZERO] }
    }
}

#[derive(Default)]
struct Trace {
    frames: Vec<f32>,
    budget: f32,
}

/// The HUD of one surface.
pub(crate) struct Hud {
    key: u64,
    view: objc2::rc::Weak<RootView>,
    monitor: Arc<PerfMonitor>,
    panel: Retained<Panel>,
    label: Retained<objc2_ui_kit::UILabel>,
    trace: Retained<TraceView>,
    process: RefCell<ProcessSampler>,
    updates: std::cell::Cell<u64>,
}

impl Hud {
    /// Shows a HUD over `view` and starts measuring its surface.
    pub(crate) fn show(view: &Retained<RootView>, mtm: MainThreadMarker) -> Rc<Hud> {
        let monitor = Arc::new(PerfMonitor::new());

        let panel_all = mtm.alloc::<Panel>().set_ivars(());
        let panel: Retained<Panel> = unsafe { msg_send![super(panel_all), initWithFrame: CGRect::ZERO] };
        panel.setUserInteractionEnabled(false);
        panel.setBackgroundColor(Some(&objc2_ui_kit::UIColor::colorWithRed_green_blue_alpha(
            0.07, 0.07, 0.08, 0.86,
        )));
        panel.setOpaque(false);
        let layer = panel.layer();
        layer.setCornerRadius(8.0);
        layer.setMasksToBounds(true);

        let label = objc2_ui_kit::UILabel::new(mtm);
        label.setNumberOfLines(0);
        // SAFETY: UIKit's colour, font and weight constructors are safe to
        // call on the main thread; objc2 marks the setters unsafe.
        unsafe {
            label.setTextColor(Some(&objc2_ui_kit::UIColor::colorWithRed_green_blue_alpha(
                0.92, 0.93, 0.95, 1.0,
            )));
            label.setFont(Some(&objc2_ui_kit::UIFont::monospacedSystemFontOfSize_weight(
                10.5,
                objc2_ui_kit::UIFontWeightRegular,
            )));
        }
        label.setBackgroundColor(Some(&objc2_ui_kit::UIColor::clearColor()));
        panel.addSubview(&label);

        let trace = TraceView::new(mtm);
        trace.setOpaque(false);
        trace.setBackgroundColor(Some(&objc2_ui_kit::UIColor::clearColor()));
        panel.addSubview(&trace);

        let hud = Rc::new(Hud {
            key: NEXT_KEY.fetch_add(1, std::sync::atomic::Ordering::Relaxed),
            view: objc2::rc::Weak::from_retained(view),
            monitor: monitor.clone(),
            panel: panel.clone(),
            label,
            trace,
            process: RefCell::new(ProcessSampler::new()),
            updates: std::cell::Cell::new(0),
        });
        // The surface's frames are timed from now on, and the engine's
        // commits are reported to the same monitor.
        log::warn!("perf[1/6]: Hud::show called");
        view.set_perf_monitor(Some(monitor.clone()));
        log::warn!("perf[2/6]: monitor handed to the surface");
        // Over the surface, not inside it: the surface's own layer holds the
        // app's views (and scrolls with them). The surface sits at the
        // container's index 0 (below development overlays), so the panel goes
        // to the container, raised over everything else.
        // The surface has a container by now: `AppView::load` inserts it
        // before the monitor starts.
        let host = view.superview();
        match &host {
            Some(host) => {
                host.addSubview(&panel);
                host.bringSubviewToFront(&panel);
            }
            None => view.addSubview(&panel),
        }
        log::warn!(
            "perf[3/6]: panel added to {} (subviews={}), hidden={}, alpha={}",
            if host.is_some() { "container" } else { "surface" },
            host.as_ref().map_or_else(|| view.subviews().len(), |h| h.subviews().len()),
            panel.isHidden(),
            panel.alpha()
        );

        HUDS.with(|h| h.borrow_mut().insert(hud.key, Rc::downgrade(&hud)));
        hud.update();
        schedule(hud.key);
        log::warn!(
            "perf[4/6]: scheduled; panel frame={}x{}@({},{})",
            panel.frame().size.width,
            panel.frame().size.height,
            panel.frame().origin.x,
            panel.frame().origin.y
        );
        hud
    }

    /// The monitor it shows.
    pub(crate) fn monitor(&self) -> &Arc<PerfMonitor> {
        &self.monitor
    }

    /// Samples the process and redraws the readings.
    fn update(&self) {
        let mut process = self.process.borrow_mut().sample();
        // GPU use is not sampled on iOS (no IO registry here).
        process.gpu = None;
        self.monitor.set_process(process);
        let snapshot = self.monitor.snapshot();

        self.label
            .setText(Some(&NSString::from_str(&readings(&snapshot))));
        let budget = 1000.0 / snapshot.refresh_rate.unwrap_or(60.0);
        *self.trace.ivars().borrow_mut() = Trace {
            frames: snapshot
                .frames
                .iter()
                .map(|d| d.as_secs_f32() * 1000.0)
                .collect(),
            budget,
        };
        self.layout();
        self.trace.setNeedsDisplay();
        let n = self.updates.get() + 1;
        self.updates.set(n);
        if n <= 12 {
            log::warn!(
                "perf[update #{}]: frame_ns={} p95_ns={} max_ns={} text={:?}",
                n,
                snapshot.frame.mean.as_nanos(),
                snapshot.frame.p95.as_nanos(),
                snapshot.frame.max.as_nanos(),
                readings(&snapshot).replace('\n', " | ")
            );
            log::warn!(
                "perf[update #{}]: fps={:.0} rate={:?} frame={:.2}ms text={:?}",
                n,
                snapshot.fps,
                snapshot.refresh_rate,
                ms(snapshot.frame.mean),
                readings(&snapshot).replace('\n', " | ")
            );
        }
    }

    /// Sizes the panel to its text, at the top right of its superview.
    fn layout(&self) {
        let Some(parent) = self.panel.superview() else {
            return;
        };
        let bounds = parent.bounds();
        log::warn!(
            "perf[layout] panel frame={:?} hidden={} alpha={} window={} hostSubviews={}",
            self.panel.frame(),
            self.panel.isHidden(),
            self.panel.alpha(),
            self.panel.window().is_some(),
            parent.subviews().len()
        );
        log::warn!(
            "perf[layout]: parent {}x{} safeTop={} labelFit={}",
            bounds.size.width,
            bounds.size.height,
            parent.safeAreaInsets().top,
            self.label
                .sizeThatFits(CGSize::new(WIDTH - 2.0 * PADDING, CGFloat::MAX))
                .height
        );
        // The safe area keeps the panel under the notch / above the home bar.
        let top = parent.safeAreaInsets().top;
        let fit = self.label.sizeThatFits(CGSize::new(WIDTH - 2.0 * PADDING, CGFloat::MAX));
        let height = PADDING * 3.0 + fit.height + GRAPH_HEIGHT;
        self.panel.setFrame(CGRect::new(
            CGPoint::new(bounds.size.width - WIDTH - MARGIN, top + MARGIN),
            CGSize::new(WIDTH, height),
        ));
        self.trace.setFrame(CGRect::new(
            CGPoint::new(PADDING, PADDING),
            CGSize::new(WIDTH - 2.0 * PADDING, GRAPH_HEIGHT),
        ));
        self.label.setFrame(CGRect::new(
            CGPoint::new(PADDING, PADDING * 2.0 + GRAPH_HEIGHT),
            CGSize::new(WIDTH - 2.0 * PADDING, fit.height),
        ));
    }
}

impl Drop for Hud {
    fn drop(&mut self) {
        self.panel.removeFromSuperview();
        if let Some(view) = self.view.load() {
            view.set_perf_monitor(None);
        }
        let key = self.key;
        let _ = HUDS.try_with(|h| h.borrow_mut().remove(&key));
    }
}

/// Redraws HUD `key` every [`INTERVAL_MS`] while it exists.
fn schedule(key: u64) {
    let when = dispatch2::DispatchTime::NOW.time((INTERVAL_MS * 1_000_000) as i64);
    let _ = dispatch2::DispatchQueue::main().after(when, move || {
        let hud = HUDS
            .try_with(|h| h.borrow().get(&key).and_then(Weak::upgrade))
            .ok()
            .flatten();
        if let Some(hud) = hud {
            hud.update();
            schedule(key);
        }
    });
}

/// Milliseconds as the panel shows them.
fn ms(d: Duration) -> f32 {
    d.as_secs_f32() * 1000.0
}

/// A duration as text: milliseconds, or microseconds when that would round
/// to `0.00` (an idle refresh costs almost nothing, and a zero reads as a
/// broken measurement).
fn format_duration(d: Duration) -> String {
    let millis = ms(d);
    if millis >= 0.01 {
        format!("{millis:.2} ms")
    } else {
        format!("{:.0} us", d.as_secs_f64() * 1e6)
    }
}

fn draw_trace(c: &CGContext, bounds: CGRect, trace: &Trace) {
    let w = bounds.size.width;
    let h = bounds.size.height;
    if w <= 0.0 || h <= 0.0 || trace.frames.is_empty() {
        return;
    }
    let n = trace.frames.len();
    let step = w / n as CGFloat;
    let budget = trace.budget.max(0.001);
    for (i, &cost) in trace.frames.iter().enumerate() {
        // Green within half the budget, amber within it, red over.
        let (r, g, b) = if cost <= budget * 0.5 {
            (0.35, 0.85, 0.45)
        } else if cost <= budget {
            (0.95, 0.78, 0.30)
        } else {
            (0.95, 0.35, 0.35)
        };
        let bar = (cost / budget).min(1.0) as CGFloat * h;
        CGContext::set_rgb_fill_color(Some(c), r, g, b, 0.9);
        CGContext::fill_rect(
            Some(c),
            CGRect::new(CGPoint::new(i as CGFloat * step, h - bar), CGSize::new(step.max(1.0), bar)),
        );
    }
    // The budget line at the top.
    CGContext::set_rgb_fill_color(Some(c), 1.0, 1.0, 1.0, 0.45);
    CGContext::fill_rect(Some(c), CGRect::new(CGPoint::new(0.0, 0.0), CGSize::new(w, 1.0)));
}

/// One line of the readings, for logs: milliseconds as `mean/p50/p95/max`.
fn log_line(s: &PerfSnapshot) -> String {
    let st = |s: &nsh_engine::perf::Stat| {
        format!(
            "{:.2}/{:.2}/{:.2}/{:.2}",
            ms(s.mean),
            ms(s.p50),
            ms(s.p95),
            ms(s.max)
        )
    };
    format!(
        "fps {:.0} ({:.0} Hz) frame {} drop {:.0}% js {} core {} {:.0} commits/s {:.0} items cpu {:.0}%",
        s.fps,
        s.refresh_rate.unwrap_or(0.0),
        st(&s.frame),
        s.dropped * 100.0,
        st(&s.js),
        st(&s.layout),
        s.commits,
        s.items,
        s.process.cpu.unwrap_or(0.0),
    )
}

/// The panel's text: one reading per line, each labelled.
fn readings(s: &PerfSnapshot) -> String {
    let rate = s
        .refresh_rate
        .map_or_else(|| "-".to_owned(), |r| format!("{r:.0} Hz"));
    let percent = |v: Option<f32>| v.map_or_else(|| "-".to_owned(), |v| format!("{v:.0}%"));
    let memory = s
        .process
        .memory
        .map_or_else(|| "-".to_owned(), |m| format!("{:.0} MB", m as f64 / 1e6));
    let budget = s
        .refresh_rate
        .map(|r| format!("{:.2} ms", 1000.0 / r.max(1.0)))
        .unwrap_or_else(|| "-".to_owned());
    format!(
        "{:.0} FPS\n\
         display {rate} · max {:.0}\n\
         frame {}\n\
         frame p95 {}\n\
         budget {budget}\n\
         drop {:.0}%\n\
         cpu {}\n\
         mem {memory}",
        s.fps,
        s.max_fps,
        format_duration(s.frame.mean),
        format_duration(s.frame.p95),
        s.dropped * 100.0,
        percent(s.process.cpu),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn readings_of_an_empty_snapshot() {
        let text = readings(&PerfSnapshot::default());
        assert!(text.starts_with("0 FPS\n"));
        assert!(text.contains("display - · max 0"));
        assert!(text.contains("drop 0%"));
        assert!(text.contains("cpu -"));
    }
}
