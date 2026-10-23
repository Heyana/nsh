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

/// The titles and icons after an `items` update: a list of titles leaves the
/// icons alone (an update resends the titles whole, the icons stay), while
/// tabs given as `{ title, icon }` carry their own.
fn merge_items(items: &[JsonValue], titles: &mut Vec<String>, icons: &mut Vec<String>) {
    *titles = items
        .iter()
        .map(|item| match item {
            JsonValue::String(title) => title.clone(),
            other => other
                .get("title")
                .and_then(|v| v.as_str())
                .unwrap_or_default()
                .to_string(),
        })
        .collect();
    if items.iter().any(|item| item.get("icon").is_some()) {
        *icons = items
            .iter()
            .map(|item| {
                item.get("icon")
                    .and_then(|v| v.as_str())
                    .unwrap_or_default()
                    .to_string()
            })
            .collect();
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
    // A width first: items made for a bar with none keep no labels or icons
    // (UIKit lays them out at the width it has when they are set).
    if bar.bounds().size.width < 1.0 {
        bar.sizeToFit();
    }
    let array = NSArray::from_retained_slice(items);
    bar.setItems(Some(&array));
    // Items made while the bar had no width keep no labels: asking for its
    // size gives it one (the core's frame still decides where it goes).
    bar.sizeToFit();
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
            self.ivars()
                .context
                // The index itself: `onSelect(index)`.
                .emit("select", serde_json::json!(item.tag() as u32));
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
    /// The tabs' titles and icons, as the app gave them (its own props
    /// arrive separately, in any order).
    titles: Vec<String>,
    icons: Vec<String>,
    context: ComponentContext,
    items: Rc<RefCell<Vec<Retained<UITabBarItem>>>>,
    mtm: MainThreadMarker,
    _delegate: Retained<TabDelegate>,
}

impl SystemTabBar {
    /// Makes the items from the titles and icons the app gave, shows them,
    /// and reports the bar's height. Its props arrive in any order, so this
    /// runs on each.
    fn rebuild(&mut self) {
        let items: Vec<_> = self
            .titles
            .iter()
            .enumerate()
            .map(|(index, title)| {
                let icon = self.icons.get(index).map(String::as_str).unwrap_or("");
                let image = (!icon.is_empty())
                    .then(|| UIImage::systemImageNamed(&NSString::from_str(icon)))
                    .flatten();
                // SAFETY: `UITabBarItem`'s initializer, with a title and
                // image it keeps.
                unsafe {
                    UITabBarItem::initWithTitle_image_tag(
                        UITabBarItem::alloc(self.mtm),
                        Some(&NSString::from_str(title)),
                        image.as_deref(),
                        index as isize,
                    )
                }
            })
            .collect();
        *self.items.borrow_mut() = items;
        let items = self.items.borrow();
        show(&self.bar, &items, self.bar.ivars().selected.get());
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
        drop(items);
    }
}

impl NativeComponent for SystemTabBar {
    fn view(&self) -> &UIView {
        &self.bar
    }

    fn set_prop(&mut self, name: &str, value: Option<&JsonValue>) {
        // A component's own props arrive as `$items` from `nsh/native`
        // (`nativeComponent`) and as `items` from a Rust component.
        let name = name.strip_prefix('$').unwrap_or(name);
        match (name, value) {
            ("items", Some(JsonValue::Array(items))) => {
                merge_items(items, &mut self.titles, &mut self.icons);
                self.rebuild();
            }
            ("icons", Some(JsonValue::Array(icons))) => {
                self.icons = icons
                    .iter()
                    .map(|icon| icon.as_str().unwrap_or_default().to_string())
                    .collect();
                self.rebuild();
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
                titles: Vec::new(),
                icons: Vec::new(),
                context: context.clone(),
                items,
                mtm,
                _delegate: delegate,
            }) as Box<dyn NativeComponent>
        }),
    );
}

#[cfg(test)]
mod tests {
    use super::merge_items;
    use serde_json::json;

    /// A title list resent with one more tab keeps the icons the app gave
    /// beside them; tabs given as objects carry their own.
    #[test]
    fn a_titles_only_update_keeps_the_icons() {
        let mut titles = Vec::new();
        let mut icons = vec!["house".to_string()];
        merge_items(&[json!("A")], &mut titles, &mut icons);
        merge_items(&[json!("A"), json!("B")], &mut titles, &mut icons);
        assert_eq!(titles, ["A", "B"]);
        assert_eq!(icons, ["house"]);

        merge_items(
            &[
                json!({ "title": "A", "icon": "star" }),
                json!({ "title": "B", "icon": "bell" }),
            ],
            &mut titles,
            &mut icons,
        );
        assert_eq!(icons, ["star", "bell"]);
    }
}
