import { Platform, signal, Text, View } from "nsh";
import { Button, Card, colors, DemoPage, hint, mono, radius } from "../ui";

/** Desktop file drop: drag files from the file manager onto the zone. */
function DropZone() {
  const hovering = signal(false);
  const uris = signal<string[]>([]);

  return (
    <Card title="Drop zone" icon="gestures">
      <View
        onFileDragEnter={() => hovering.set(true)}
        onFileDragLeave={() => hovering.set(false)}
        onFileDrop={(e) => {
          uris.set(e.uris);
          hovering.set(false);
        }}
        style={{
          minHeight: 160,
          borderRadius: radius.control,
          borderWidth: 1,
          borderStyle: "dashed",
          borderColor: hovering() ? colors.accent : colors.border,
          backgroundColor: hovering() ? colors.accentSoft : colors.background,
          alignItems: "center",
          justifyContent: "center",
          padding: 24,
          gap: 8,
        }}
      >
        <Text style={{ fontSize: 15, color: colors.text, textAlign: "center" }}>
          {hovering() ? "Release to receive the files" : "Drag files here from your file manager"}
        </Text>
        <Text style={{ fontSize: 12, fontFamily: mono, color: colors.faint, textAlign: "center" }}>
          onFileDrop · onFileDragEnter · onFileDragLeave
        </Text>
      </View>
      <View style={{ flexDirection: "row", alignItems: "center", gap: 8, marginTop: 12 }}>
        <Text style={{ fontSize: 13, color: colors.muted }}>
          {uris().length === 0 ? "No files dropped yet" : `${uris().length} file${uris().length > 1 ? "s" : ""} received`}
        </Text>
        <View style={{ flex: 1 }} />
        {uris().length > 0 && <Button title="Clear" onPress={() => uris.set([])} />}
      </View>
      {uris().length > 0 && (
        <View
          style={{
            marginTop: 8,
            borderRadius: radius.control,
            backgroundColor: colors.background,
            borderWidth: 1,
            borderColor: colors.border,
            padding: 12,
            gap: 4,
          }}
        >
          {uris().map((uri) => (
            <Text key={uri} style={{ fontSize: 12, fontFamily: mono, color: colors.text }} numberOfLines={1}>
              {uri}
            </Text>
          ))}
        </View>
      )}
    </Card>
  );
}

function NotDesktop() {
  return (
    <Card title="Desktop only" icon="system">
      <Text style={{ fontSize: 14, color: colors.muted }}>
        File drag and drop needs a desktop file manager. Run this demo with{" "}
        <Text style={{ fontFamily: mono }}>nsh run --target linux</Text> (or macos / windows).
      </Text>
    </Card>
  );
}

export function FileDrop() {
  return (
    <DemoPage
      cards={[
        () => (
          <>
            <Text style={{ fontSize: 13, color: colors.muted, marginBottom: 12 }}>
              {hint(
                "File drop needs a desktop file manager.",
                "A view with onFileDrop becomes a native drop destination: AppKit, GTK and Win32 route the drop through hit testing."
              )}
            </Text>
            {Platform.isDesktop ? <DropZone /> : <NotDesktop />}
          </>
        ),
      ]}
    />
  );
}
