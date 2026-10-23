//! `nsh/tabbar`: the platform's tab bar, a `UITabBar`. iOS 26 draws it with
//! Liquid Glass, its floating capsule and selection pill, and shrinks it as
//! the app's content scrolls, as the system's own tab bar does.
//!
//! `nsh/base`'s `TabBar` renders it through `native("nsh/tabbar")` and wires
//! its `select` event; the app decides what each tab shows.

use std::{
    cell::{Cell, RefCell},
    rc::Rc,
};

use nsh_core::JsonValue;
use objc2::{
    DefinedClass, MainThreadMarker, MainThreadOnly, define_class, msg_send, rc::Retained,
    runtime::ProtocolObject, sel,
};
use objc2_core_foundation::{CGPoint, CGRect, CGSize};
use objc2_foundation::{NSArray, NSObject, NSObjectProtocol, NSString};
use objc2_ui_kit::{UIImage, UIResponder, UITabBar, UITabBarDelegate, UITabBarItem, UIView};

use crate::{
    AppView,
    component::{ComponentContext, NativeComponent},
};

/// What the delegate and the bar share: the items to show and which is
/// selected, for the bar to lay them out again when it gets its width.
struct BarIvars {
    items: Rc<RefCell<Vec<Retained<UITabBarItem>>>>,
    selected: Cell<usize>,
    /// The width the items were last laid out at: an element is made before
    /// the layout that sizes it, so the first `setItems:` lands at width 0
    /// and draws nothing until the bar is laid out again.
    width: Cell<f64>,
}

define_class!(
    /// A `UITabBar` that shows its items once it has a width to lay them out
    /// in.
    #[unsafe(super(UITabBar, UIView, UIResponder, NSObject))]
    #[thread_kind = MainThreadOnly]
    #[ivars = BarIvars]
    struct NshTabBar;

    impl NshTabBar {
        /// Shows the items, once the bar has a width (a turn after the
        /// layout that gave it one).
        #[unsafe(method(nshShowItems))]
        fn nsh_show_items(&self) {
            show(self, &self.ivars().items.borrow(), self.ivars().selected.get());
        }

        /// A bar makes its item views once it is in a window: the items set
        /// before that draw nothing.
        #[unsafe(method(didMoveToWindow))]
        fn did_move_to_window(&self) {
            let _: () = unsafe { msg_send![super(self), didMoveToWindow] };
            show(self, &self.ivars().items.borrow(), self.ivars().selected.get());
        }

        #[unsafe(method(layoutSubviews))]
        fn layout_subviews(&self) {
            // SAFETY: the superclass' own layout.
            let _: () = unsafe { msg_send![super(self), layoutSubviews] };
            let width = self.bounds().size.width;
            if width < 1.0 || width == self.ivars().width.get() {
                return;
            }
            self.ivars().width.set(width);
            // Items laid out while the bar had no width keep no labels: give
            // them up and set them again for the width it has now.
            self.setItems(None);
            // After the layout pass: items set inside it are not laid out.
            let _: () = unsafe {
                msg_send![
                    self,
                    performSelector: sel!(nshShowItems),
                    withObject: std::ptr::null::<NSObject>(),
                    afterDelay: 0.0f64
                ]
            };
        }
    }
);

impl NshTabBar {
    fn new(
        mtm: MainThreadMarker,
        items: Rc<RefCell<Vec<Retained<UITabBarItem>>>>,
    ) -> Retained<Self> {
        let this = Self::alloc(mtm).set_ivars(BarIvars {
            items,
            selected: Cell::new(0),
            width: Cell::new(0.0),
        });
        // SAFETY: `UITabBar`'s initializer is `UIView`'s `initWithFrame:`;
        // a bar made with `init` has no item views. The element's layout
        // sets the frame again.
        let frame = CGRect::new(CGPoint::new(0.0, 0.0), CGSize::new(320.0, 49.0));
        unsafe { msg_send![super(this), initWithFrame: frame] }
    }
}

/// Shows `items` on `bar`, with `selected` chosen.
fn show(bar: &UITabBar, items: &[Retained<UITabBarItem>], selected: usize) {
    if items.is_empty() {
        // An empty list empties the bar: the items it holds belong to a list
        // that is gone, and a bar left holding them keeps them showing and
        // tappable.
        let none = NSArray::new();
        bar.setItems(Some(&none));
        bar.setSelectedItem(None);
        return;
    }
    let array = NSArray::from_retained_slice(items);
    bar.setItems(Some(&array));
    // A bar has no size of its own until it is asked for one: the element's
    // frame is the box the app sized, the bar sits at its bottom.
    bar.sizeToFit();
    if let Some(container) = bar.superview() {
        let bounds = container.bounds();
        let natural = bar.sizeThatFits(CGSize::new(bounds.size.width, f64::INFINITY));
        let height = natural.height;
        let insets = container.safeAreaInsets();
        bar.setFrame(CGRect::new(
            CGPoint::new(0.0, bounds.size.height - height - insets.bottom),
            CGSize::new(bounds.size.width, height),
        ));
    }
    bar.setNeedsLayout();
    bar.layoutIfNeeded();
    if let Some(item) = items.get(selected) {
        bar.setSelectedItem(Some(item));
    }
}

struct DelegateIvars {
    context: ComponentContext,
}

define_class!(
    /// Reports the tab the user chose, by its index (the item's tag).
    #[unsafe(super(NSObject))]
    #[thread_kind = MainThreadOnly]
    #[ivars = DelegateIvars]
    struct TabDelegate;

    unsafe impl NSObjectProtocol for TabDelegate {}

    unsafe impl UITabBarDelegate for TabDelegate {
        #[unsafe(method(tabBar:didSelectItem:))]
        fn did_select(&self, _bar: &UITabBar, item: &UITabBarItem) {
            log::info!("nsh/tabbar: selected {}", item.tag());
            self.ivars()
                .context
                .emit("select", serde_json::json!({ "index": item.tag() as u32 }));
        }
    }
);

impl TabDelegate {
    fn new(mtm: MainThreadMarker, context: ComponentContext) -> Retained<Self> {
        let this = Self::alloc(mtm).set_ivars(DelegateIvars { context });
        // SAFETY: NSObject's designated initializer.
        unsafe { msg_send![super(this), init] }
    }
}

/// The tab bar of one `nsh/tabbar` element.
pub(crate) struct SystemTabBar {
    bar: Retained<NshTabBar>,
    context: ComponentContext,
    items: Rc<RefCell<Vec<Retained<UITabBarItem>>>>,
    mtm: MainThreadMarker,
    _delegate: Retained<TabDelegate>,
}

impl NativeComponent for SystemTabBar {
    fn view(&self) -> &UIView {
        &self.bar
    }

    fn set_prop(&mut self, name: &str, value: Option<&JsonValue>) {
        match (name, value) {
            ("items", Some(JsonValue::Array(items))) => {
                let parsed: Vec<_> = items
                    .iter()
                    .enumerate()
                    .map(|(index, item)| {
                        let title = item
                            .get("title")
                            .and_then(|v| v.as_str())
                            .unwrap_or_default();
                        let icon = item
                            .get("icon")
                            .and_then(|v| v.as_str())
                            .unwrap_or_default();
                        let image = (!icon.is_empty())
                            .then(|| UIImage::systemImageNamed(&NSString::from_str(icon)))
                            .flatten();
                        let item = unsafe {
                            UITabBarItem::initWithTitle_image_tag(
                                UITabBarItem::alloc(self.mtm),
                                Some(&NSString::from_str(title)),
                                image.as_deref(),
                                index as isize,
                            )
                        };
                        item
                    })
                    .collect();
                *self.items.borrow_mut() = parsed;
                let items = self.items.borrow();
                show(&self.bar, &items, self.bar.ivars().selected.get());
                // The bar's own height: the app leaves it open and places the
                // element at the bottom.
                let width = self
                    .bar
                    .superview()
                    .map(|container| container.bounds().size.width)
                    .filter(|width| *width > 1.0)
                    .unwrap_or(393.0);
                let natural = self
                    .bar
                    .sizeThatFits(CGSize::new(width, f64::INFINITY))
                    .height;
                self.context.set_size(0.0, natural as f32);
            }
            ("selected", Some(JsonValue::Number(index))) => {
                let selected = index.as_u64().unwrap_or(0) as usize;
                self.bar.ivars().selected.set(selected);
                let items = self.items.borrow();
                show(&self.bar, &items, selected);
            }
            _ => {}
        }
    }
}

/// Registers the component: `nsh/base`'s `TabBar` names it.
pub(crate) fn register(view: &AppView) {
    view.register_component(
        "nsh/tabbar",
        Rc::new(|context: ComponentContext| {
            let mtm = MainThreadMarker::new().expect("the tab bar is made on the main thread");
            let items = Rc::new(RefCell::new(Vec::new()));
            let bar = NshTabBar::new(mtm, items.clone());
            let delegate = TabDelegate::new(mtm, context.clone());
            bar.setDelegate(Some(ProtocolObject::from_ref(&*delegate)));
            Box::new(SystemTabBar {
                bar,
                context: context.clone(),
                items,
                mtm,
                _delegate: delegate,
            }) as Box<dyn NativeComponent>
        }),
    );
}
