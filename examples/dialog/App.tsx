// A desktop app showing the system file picker: `Dialog.openFile` and
// `Dialog.saveFile` from `nsh/dialog`. Desktop-only; elsewhere the
// promises resolve to null.

import { mount, signal, Text, View } from "nsh";
import { Button } from "nsh/ui/mobile";
import { Dialog } from "nsh/dialog";

function App() {
  const result = signal("No file chosen yet.");

  const openOne = async () => {
    const paths = await Dialog.openFile({ title: "Open a file" });
    result.set(paths ? `Open: ${paths.join(", ")}` : "Open cancelled.");
  };

  const openMany = async () => {
    const paths = await Dialog.openFile({
      title: "Open files",
      multiple: true,
      filters: ["*.png", "*.jpg"],
    });
    result.set(paths ? `Open (${paths.length}): ${paths.join(", ")}` : "Open cancelled.");
  };

  const save = async () => {
    const path = await Dialog.saveFile({
      title: "Save file",
      defaultName: "untitled.txt",
    });
    result.set(path ? `Save: ${path}` : "Save cancelled.");
  };

  return (
    <View style={{ flex: 1, padding: 24, gap: 12 }}>
      <Text style={{ fontSize: 20, fontWeight: "600" }}>File picker</Text>
      <Button title="Open a file…" onPress={openOne} />
      <Button title="Open images… (multiple)" onPress={openMany} />
      <Button title="Save as…" onPress={save} />
      <Text style={{ marginTop: 12 }}>{result()}</Text>
    </View>
  );
}

mount(App);
