---
title: nsh/ios
description: "iOS-only APIs: isAvailable, TabBar and InputAccessoryView."
order: 11
---

# nsh/ios

`nsh/ios` holds APIs that only iOS has. Rust code builds the same bar with `input_accessory_view()` of `nsh_core::ui::prelude` and checks `os() == Os::Ios`; TSX imports the module, which can be imported on every platform, so one bundle runs everywhere: elsewhere its components render nothing, and `isAvailable` tells you whether you are on iOS.

```rust tabs
use nsh_core::{platform::Os, ui::prelude::*}; // os, input_accessory_view
```

```tsx tabs
import { InputAccessoryView, isAvailable } from "nsh/ios";
```

iOS-only *props* of elements and components are not here: they go in the `ios={{…}}` prop in TSX (for example `<Input ios={{ textContentType: "oneTimeCode" }} />`). See [Platform-specific code](../docs/platform-specific.md).

## isAvailable

```ts
const isAvailable: boolean;
```

`true` on iOS, the iOS simulator and Mac Catalyst; `false` elsewhere. Rust: `os() == Os::Ios`.

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

UIKit's system tab bar, placed by the app. `items` supplies titles in order;
`icons` supplies SF Symbol names at the same indices. `selected` defaults to 0,
and `onSelect(index)` reports the tab the user chooses. Update `selected` in
that callback to follow the user's choice. Removing `icons` clears the symbols;
removing `selected` restores index 0.

The bar uses the system appearance for the current iOS version. Its container
sets its frame from `style` and `class`; its intrinsic height is measured at the
laid-out width when height is automatic. The app places the bar and its content
and applies safe-area insets for its layout. This component provides the bar;
scrolling behavior belongs to the app's content and navigation.

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

A bar over the keyboard (UIKit's `inputAccessoryView`), shown while an input whose `ios={{ inputAccessoryViewID }}` matches its `nativeID` has focus. Rust: `input_accessory_view(native_id)`, shown for the inputs whose `.input_accessory(native_id)` matches. It is as wide as the window and as tall as its content, and moves with the keyboard natively.

| Prop | Type | Rust | Description |
|---|---|---|---|
| `nativeID` | `string` | `input_accessory_view(native_id)` | Inputs with this `inputAccessoryViewID` show the bar. |
| `style` | `StyleProp<ViewStyle>` | `Styled` methods (`.bg(…)`) | The bar's style (its background, for example). |
| `class` | `string` | | Tailwind classes. |
| `children` | `JSX.Element` | `.child(…)` | The bar's content. |

It renders nothing where it is placed in the tree, and nothing at all on Android, so you can place it next to the input it belongs to.

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

See also [Forms and keyboard](../docs/forms-and-keyboard.md).
