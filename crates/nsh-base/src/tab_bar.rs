//! `TabBar`: the platform's tab bar. On iOS the system draws it (a
//! `UITabBar`, with Liquid Glass, its floating capsule and selection pill,
//! shrunk as the content scrolls); elsewhere it is drawn from views, and the
//! app styles it as it styles `Tabs`.

use nsh_core::{
    platform::Os,
    ui::{AnyElement, NativeEvent, ambient::os, each, native, prelude::*},
};

/// The props of [`TabBar`].
#[derive(Debug, Clone, PartialEq, Props)]
pub struct TabBarProps {
    /// The tabs' titles, in order.
    pub items: Prop<Vec<SharedString>>,
    /// The icon of each tab: an SF Symbol's name (iOS). Empty for a title
    /// alone.
    #[prop(default)]
    pub icons: Prop<Vec<SharedString>>,
    /// The selected tab, by index. The app owns it: `onSelect` reports the
    /// one the user chose, and the app shows that tab's content.
    #[prop(default)]
    pub selected: Prop<u32>,
}

/// The events of [`TabBar`].
#[derive(Debug, Clone, PartialEq, Event)]
pub enum TabBarEvent {
    /// The user chose a tab: its index.
    #[args(index)]
    Select(u32),
}

/// A tab bar: a row of a few top-level sections, one selected, that switches
/// between them. On iOS it is the system's (its look and behavior are the
/// system's; the app places and sizes it); elsewhere it is drawn, with the
/// app's `class` and `style`.
#[component]
pub fn TabBar(props: TabBarProps, cx: Scope<TabBarEvent>) -> AnyElement {
    if os() == Os::Ios {
        system(props, cx)
    } else {
        drawn(props, cx)
    }
}

/// The system's bar: its items, the selected one, and its `select` event.
fn system(props: TabBarProps, cx: Scope<TabBarEvent>) -> AnyElement {
    let items = props.items.clone();
    let icons = props.icons.clone();
    let selected = props.selected.clone();
    native("nsh/tabbar")
        .prop("items", move || {
            let items = items.get();
            let icons = icons.get();
            serde_json::Value::Array(
                items
                    .iter()
                    .enumerate()
                    .map(|(index, title)| {
                        serde_json::json!({
                            "title": &**title,
                            "icon": icons.get(index).map(|icon| &**icon).unwrap_or_default(),
                        })
                    })
                    .collect(),
            )
        })
        .prop("selected", move || serde_json::json!(selected.get()))
        .on_native_event(move |event: &NativeEvent| {
            if &*event.name == "select" {
                if let Some(index) = event.data.get("index").and_then(|index| index.as_u64()) {
                    cx.emit(TabBarEvent::Select(index as u32));
                }
            }
        })
        .into_element()
}

/// The drawn bar: the titles in a row, the selected one marked, a press on
/// each.
fn drawn(props: TabBarProps, cx: Scope<TabBarEvent>) -> AnyElement {
    let selected = props.selected;
    view()
        .flex_row()
        .child(each(
            props.items,
            |title| title.clone(),
            move |title: ReadSignal<SharedString>, index: ReadSignal<usize>| {
                view()
                    .flex_1()
                    .items_center()
                    .py_2()
                    // Reactive: `each` keeps a row whose key did not change,
                    // so the marker has to follow `selected` itself.
                    .child(text(move || {
                        if index.get() == selected.get() as usize {
                            SharedString::from(format!("• {}", title.get()))
                        } else {
                            title.get()
                        }
                    }))
                    .on_press(move |_: &()| cx.emit(TabBarEvent::Select(index.get() as u32)))
                    .into_element()
            },
        ))
        .into_element()
}
