import { mount, signal, Text, View } from "nsh";
import { Menu } from "nsh/desktop";

/**
 * Right-click the card to open a context menu.
 * `onContextMenu` fires with the click position in the view's points;
 * `Menu.popup` shows the native menu there.
 */
function App() {
  const last = signal("Right-click the card");

  const items = [
    { id: "copy", label: "Copy" },
    { id: "paste", label: "Paste" },
    null,
    { id: "delete", label: "Delete", role: "destructive" },
  ];

  return (
    <View style={{ flex: 1, alignItems: "center", justifyContent: "center" }}>
      <View
        onContextMenu={(e) => {
          last.set(`Menu at ${Math.round(e.x)}, ${Math.round(e.y)}`);
          Menu.popup(items, { x: e.x, y: e.y });
        }}
        style={{
          padding: 24,
          borderRadius: 8,
          borderWidth: 1,
          borderColor: "#ccc",
        }}
      >
        <Text>{last()}</Text>
      </View>
    </View>
  );
}

mount(App);
