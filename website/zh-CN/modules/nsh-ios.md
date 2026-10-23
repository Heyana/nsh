---
title: nsh/ios
description: iOS 专有 API：isAvailable、TabBar 和 InputAccessoryView。
order: 11
---

# nsh/ios

`nsh/ios` 包含只有 iOS 才有的 API。Rust 代码用 `nsh_core::ui::prelude` 中的 `input_accessory_view()` 构建同样的工具栏，并用 `os() == Os::Ios` 判断平台；TSX 导入这个模块，它在所有平台上都可以导入，因此同一个 bundle 可以在所有平台运行：在其他平台上它的组件什么也不渲染，`isAvailable` 告诉你当前是否在 iOS 上。

```rust tabs
use nsh_core::{platform::Os, ui::prelude::*}; // os、input_accessory_view
```

```tsx tabs
import { InputAccessoryView, isAvailable } from "nsh/ios";
```

元素和组件的 iOS 专有*属性*不在这里：在 TSX 中它们放在 `ios={{…}}` 属性中（例如 `<Input ios={{ textContentType: "oneTimeCode" }} />`）。见[平台专有代码](../docs/platform-specific.md)。

## isAvailable

```ts
const isAvailable: boolean;
```

在 iOS、iOS 模拟器和 Mac Catalyst 上为 `true`，其他平台为 `false`。Rust：`os() == Os::Ios`。

## TabBar

```ts
function TabBar(props: TabBarProps): YNode | null;

interface TabBarProps {
  items: string[];
  icons?: string[];
  selected?: number;
  onSelect?: (index: number) => void;
  style?: StyleProp<ViewStyle>;
  class?: string;
}
```

UIKit 的系统标签栏，由应用决定放置位置。`items` 按顺序提供标题，
`icons` 在相同索引提供 SF Symbol 名称。`selected` 默认为 0，
`onSelect(index)` 报告用户选择的标签。在回调中更新 `selected` 即可跟随用户选择。
移除 `icons` 会清空图标；移除 `selected` 会恢复索引 0。

标签栏采用当前 iOS 版本的系统外观。容器根据 `style` 和 `class` 设置其 frame；
高度为自动时，使用布局后的实际宽度测量固有高度。应用负责标签栏及其内容的位置，
并根据布局应用安全区域边距。此组件提供标签栏，滚动行为由应用的内容和导航管理。

```tsx
import { signal } from "nsh";
import { TabBar } from "nsh/ios";

const selected = signal(0);

<TabBar
  items={["Records", "Stats"]}
  icons={["list.bullet.rectangle", "chart.bar"]}
  selected={selected()}
  onSelect={(index) => selected.set(index)}
  style={{ position: "absolute", left: 0, right: 0, bottom: 0 }}
/>;
```

## InputAccessoryView

```ts
function InputAccessoryView(props: InputAccessoryViewProps): YNode | null;

interface InputAccessoryViewProps {
  nativeID: string;
  style?: StyleProp<ViewStyle>;
  class?: string;
  children?: JSX.Element;
}
```

键盘上方的工具栏（UIKit 的 `inputAccessoryView`），当 `ios={{ inputAccessoryViewID }}` 与它的 `nativeID` 相同的输入框获得焦点时显示。Rust：`input_accessory_view(native_id)`，在 `.input_accessory(native_id)` 与之相同的输入框获得焦点时显示。它与窗口等宽、高度由内容决定，并在原生侧随键盘一起移动。

| 属性 | 类型 | Rust | 说明 |
|---|---|---|---|
| `nativeID` | `string` | `input_accessory_view(native_id)` | `inputAccessoryViewID` 为该值的输入框会显示这个工具栏。 |
| `style` | `StyleProp<ViewStyle>` | `Styled` 方法（`.bg(…)`） | 工具栏的样式（例如背景色）。 |
| `class` | `string` | | Tailwind class。 |
| `children` | `JSX.Element` | `.child(…)` | 工具栏的内容。 |

它在树中所处的位置不渲染任何内容，在 Android 上则完全不渲染，因此可以把它放在所属输入框的旁边。

```rust tabs
use nsh_core::ui::prelude::*;

#[component]
fn Note() -> impl IntoElement {
    let note = signal(String::new());
    let field = element_ref();
    view()
        .child(
            input()
                .element_ref(&field)
                .value(note)
                .on_change(move |value: &str| note.set(value.to_string()))
                .placeholder("Write a note")
                .input_accessory("note-bar"),
        )
        .child(
            input_accessory_view("note-bar").bg(rgb(0xf1f5f9)).child(
                view()
                    .flex_row()
                    .items_center()
                    .p(px(10.0))
                    .child(
                        text(move || format!("{} characters", note.with(|n| n.chars().count())))
                            .flex_1()
                            .text_color(rgb(0x64748b)),
                    )
                    .child(text("Done").font_weight(FontWeight::SEMIBOLD).on_press(move || field.blur())),
            ),
        )
}
```

```tsx tabs
import { Input, signal, Text, View, type YNode } from "nsh";
import { InputAccessoryView } from "nsh/ios";

function Note() {
  const text = signal("");
  let input: YNode | undefined;
  return (
    <View>
      <Input
        ref={(n) => (input = n)}
        value={text()}
        onChange={(v) => text.set(v)}
        placeholder="Write a note"
        ios={{ inputAccessoryViewID: "note-bar" }}
      />
      <InputAccessoryView nativeID="note-bar" style={{ backgroundColor: "#f1f5f9" }}>
        <View style={{ flexDirection: "row", alignItems: "center", padding: 10 }}>
          <Text style={{ flex: 1, color: "#64748b" }}>{text().length} characters</Text>
          <Text onPress={() => input?.blur()} style={{ fontWeight: "600" }}>Done</Text>
        </View>
      </InputAccessoryView>
    </View>
  );
}
```

另见[表单与键盘](../docs/forms-and-keyboard.md)。
