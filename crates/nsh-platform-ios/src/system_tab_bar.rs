//! `nsh/ios`'s `TabBar`: a `UITabBar` with the system appearance.
//! The app places it and decides what each tab shows; the component reports
//! its intrinsic height while its container owns its frame.

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
    height: Cell<f64>,
    context: ComponentContext,
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
            // SAFETY: forwarding UIView's callback to its superclass on the main thread.
            let _: () = unsafe { msg_send![super(self), didMoveToWindow] };
            show(self, &self.ivars().items.borrow(), self.ivars().selected.get());
        }

        #[unsafe(method(layoutSubviews))]
        fn layout_subviews(&self) {
            // UIKit can give the bar its natural height before the host is
            // sized. Flexible-height autoresizing then adds the host's height
            // to that initial size; fill the host exactly instead.
            if let Some(host) = self.superview() {
                let frame = host.bounds();
                if self.frame() != frame {
                    self.setFrame(frame);
                }
            }
            // SAFETY: the superclass' own layout.
            let _: () = unsafe { msg_send![super(self), layoutSubviews] };
            let width = self.bounds().size.width;
            let height = self.sizeThatFits(CGSize::new(width, f64::INFINITY)).height;
            if height != self.ivars().height.replace(height) {
                self.ivars().context.set_size(0.0, height as f32);
            }
            if width < 1.0 || width == self.ivars().width.get() {
                return;
            }
            self.ivars().width.set(width);
            // Items laid out while the bar had no width keep no labels: give
            // them up and set them again for the width it has now.
            self.setItems(None);
            // After the layout pass: items set inside it are not laid out.
            // SAFETY: this zero-argument selector is defined on this class above.
            // NSObject retains the receiver until the main run loop invokes it.
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
        context: ComponentContext,
    ) -> Retained<Self> {
        let this = Self::alloc(mtm).set_ivars(BarIvars {
            items,
            selected: Cell::new(0),
            width: Cell::new(0.0),
            height: Cell::new(0.0),
            context,
        });
        // SAFETY: `UITabBar`'s initializer is `UIView`'s `initWithFrame:`;
        // a bar made with `init` has no item views. The element's layout
        // sets the frame again.
        let frame = CGRect::new(CGPoint::new(0.0, 0.0), CGSize::new(0.0, 0.0));
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
    let array = NSArray::from_retained_slice(items);
    bar.setItems(Some(&array));
    bar.setNeedsLayout();
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
    items: Rc<RefCell<Vec<Retained<UITabBarItem>>>>,
    mtm: MainThreadMarker,
    _delegate: Retained<TabDelegate>,
}

impl SystemTabBar {
    /// Makes the items from the titles and icons the app gave, shows them,
    /// with the current selection. Its props arrive in any order, so this
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
                UITabBarItem::initWithTitle_image_tag(
                    UITabBarItem::alloc(self.mtm),
                    Some(&NSString::from_str(title)),
                    image.as_deref(),
                    index as isize,
                )
            })
            .collect();
        *self.items.borrow_mut() = items;
        let items = self.items.borrow();
        show(&self.bar, &items, self.bar.ivars().selected.get());
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
            ("icons", None | Some(JsonValue::Null)) => {
                self.icons.clear();
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
                self.bar
                    .setSelectedItem(items.get(selected).map(|item| &**item));
            }
            ("selected", None | Some(JsonValue::Null)) => {
                self.bar.ivars().selected.set(0);
                let items = self.items.borrow();
                self.bar.setSelectedItem(items.first().map(|item| &**item));
            }
            _ => {}
        }
    }
}

/// Registers the component used by `nsh/ios`'s `TabBar`.
pub(crate) fn register(view: &AppView) {
    view.register_component(
        "nsh/tabbar",
        Rc::new(|context: ComponentContext| {
            let mtm = MainThreadMarker::new().expect("the tab bar is made on the main thread");
            let items = Rc::new(RefCell::new(Vec::new()));
            let bar = NshTabBar::new(mtm, items.clone(), context.clone());
            let delegate = TabDelegate::new(mtm, context.clone());
            bar.setDelegate(Some(ProtocolObject::from_ref(&*delegate)));
            Box::new(SystemTabBar {
                bar,
                titles: Vec::new(),
                icons: Vec::new(),
                items,
                mtm,
                _delegate: delegate,
            }) as Box<dyn NativeComponent>
        }),
    );
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::merge_items;

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
