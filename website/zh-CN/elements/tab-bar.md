---
title: TabBar
description: "TabBar 组件：系统的标签栏，iOS 上是 UITabBar，Android 上是 Material 3 导航栏，外观都是系统的。应用提供标签、掌握选中的那一个，并显示它的内容。"
order: 20.65
group: base
---

# TabBar

标签栏在应用的几个顶层部分之间切换：记录和统计、收件箱和设置。`TabBar` 就是系统自己的标签栏，所以它的外观和行为与系统应用一致：iOS 和 iPadOS 上是 `UITabBar`（iOS 26 上是 Liquid Glass 及其选中效果），Android 上是 Material 3 的导航栏，使用系统的颜色，浅色和深色都一样。应用提供标签（`items`），指明选中哪一个（`selectedIndex`），接收用户的选择（`onChange`），用 `class` 和 `style` 摆放标签栏，并自己显示选中部分的内容。

```rust tabs
use nsh_base::{TabBarItem, tab_bar};
use nsh_core::ui::prelude::*;

fn sections() -> impl IntoElement {
    let section = signal(0u32);
    view()
        .flex_1()
        .child(text(move || match section.get() {
            0 => "Records",
            _ => "Stats",
        }))
        .child(
            tab_bar()
                .items(vec![
                    TabBarItem::new("Records").icon("list.bullet"),
                    TabBarItem::new("Stats").icon("chart.bar"),
                ])
                .selected_index(section)
                .on_change(move |index: &u32| section.set(*index))
                .absolute()
                .left_0()
                .right_0()
                .bottom_0(),
        )
}
```

```tsx tabs
import { mount, signal, Text, View } from "nsh";
import { TabBar } from "nsh/base";
import { platform } from "nsh/device";

function App() {
  const section = signal(0);
  return (
    <View class="flex-1">
      <Text>{section() === 0 ? "Records" : "Stats"}</Text>
      <TabBar
        items={[
          { title: "Records", icon: platform.select({ ios: "list.bullet", android: "ic_records" }) },
          { title: "Stats", icon: platform.select({ ios: "chart.bar", android: "ic_stats" }) },
        ]}
        selectedIndex={section()}
        onChange={(index) => section.set(index)}
        class="absolute inset-x-0 bottom-0"
      />
    </View>
  );
}

mount(App);
```

`TabBar` 来自 [`nsh/base`](../modules/nsh-base.md)，在 Rust 中是 `nsh-base` crate。选中状态属于应用：一次点按通过 `onChange` 报告该标签的索引，标签栏像系统的一样立即显示这次选择，应用显示那一部分并把索引作为 `selectedIndex` 传回。样式没有给定高度时，高度是系统的。在 Rust 中，`tab_bar()` 创建它，每个 prop 一个 setter（`.items(..)`、`.selected_index(..)`），每个事件一个监听方法（`.on_change(..)`），以及带类型的样式方法；`TabBarItem::new(title).icon(name)` 创建一个标签。

## Props

| Prop | Rust | 类型 | 默认值 | 说明 |
|---|---|---|---|---|
| `items` | `.items(..)` | `TabBarItem[]` | `[]` | 各个标签，按顺序。 |
| `selectedIndex` | `.selected_index(..)` | `number` | `0` | 选中的标签的索引；超出标签数量的索引不选中任何标签。 |
| `class`、`style` | 样式方法 | `string`、`StyleProp<ViewStyle>` | 无 | 标签栏的位置和尺寸。外观是系统的。 |
| `testID` | `.test_id(..)` | `string` | 无 | 在测试和 `nsh test --trace` 中找到标签栏。 |

### TabBarItem

| 字段 | Rust | 类型 | 说明 |
|---|---|---|---|
| `title` | `TabBarItem::new(title)` | `string` | 标签的标题，屏幕阅读器会读出它。 |
| `icon` | `.icon(name)` | `string` | 标签的图标，用平台对它的称呼：iOS 上是 [SF Symbol](https://developer.apple.com/sf-symbols/)（`"house"`），Android 上是应用的 drawable（`res/drawable/home.xml` 写作 `"home"`，`"android:ic_menu_search"` 是系统的）。用 `platform.select` 按平台选择。 |

## 事件

| 事件 | Rust | 说明 |
|---|---|---|
| `onChange` | `.on_change(\|index: &u32\| ..)` | 用户选择了一个标签：它的索引。标签栏立即显示这次选择；应用显示该标签的内容并设置 `selectedIndex`。 |

## 平台说明

- **iOS 和 Mac Catalyst**：`UITabBar`，采用当前 iOS 版本的系统外观：iOS 26 上是悬浮的 Liquid Glass 标签栏及其选中效果。图标是 SF Symbols。
- **Android**：Material 3 的导航栏，由平台的 View 组成：每个标签的文字上方是放在激活指示器中的图标，按压时有水波纹；Android 12 及以上使用系统的动态颜色，更早的版本使用 Material 的基准颜色；浅色或深色跟随应用的外观。位于窗口底部时，它延伸到导航栏下方，高度增加导航栏的 inset，与 Material 的一致。图标是应用的 drawable（由标签栏着色）。
- **桌面（macOS、Windows、Linux、GPUI）和 Web**：各个部分用 [Tabs](./tabs.md) 显示，外观由应用决定。`TabBar` 在这些平台上不占空间，所以一个布局可以同时包含两者，用 `platform.os`（[nsh/device](../modules/nsh-device.md)）来选择。

页面内的标签（一个项目的概览和动态），以及在所有平台上，都请使用 [Tabs](./tabs.md)。
