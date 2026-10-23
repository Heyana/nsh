// A case of the API's test suite (docs/dev/compatibility.md): `nsh/ios` on
// the headless runtime, which is iOS: `isAvailable` is true, and an
// `InputAccessoryView` takes no room where it is placed. Its script checks each.
import { Input, Text, View, signal } from "nsh";
import { InputAccessoryView, TabBar, isAvailable } from "nsh/ios";

function App() {
  const selected = signal(0);
  const icons = signal<string[] | undefined>(["list.bullet", "chart.bar"]);
  const bar = (
    <TabBar
      items={["Records", "Stats"]}
      icons={icons()}
      selected={selected()}
      onSelect={(index) => selected.set(index)}
      style={{ position: "absolute", left: 0, right: 0, bottom: 0, height: 60 }}
    />
  );
  return (
    <View style={{ flex: 1, paddingTop: 64 }}>
      <Text>{`iOS: ${isAvailable}`}</Text>
      <View testID="column">
        <View testID="above" style={{ height: 30 }} />
        <InputAccessoryView nativeID="bar">
          <View style={{ height: 44 }}>
            <Text>Accessory bar</Text>
          </View>
        </InputAccessoryView>
        <View testID="below" style={{ height: 30 }} />
      </View>
      <Input ios={{ inputAccessoryViewID: "bar" }} style={{ height: 40 }} />
      <Text>Rendered</Text>
      {bar}
      <Text>{`Tab bar: ${bar !== null}`}</Text>
      <Text>{`Tab: ${selected()}`}</Text>
      <Text testID="tab-next" onPress={() => { selected.set(1); icons.set(undefined); }}>Next tab</Text>
    </View>
  );
}

export default App;
