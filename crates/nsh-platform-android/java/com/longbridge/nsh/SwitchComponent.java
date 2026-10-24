package com.longbridge.nsh;

import android.content.Context;
import android.view.View;
import android.widget.Switch;

/**
 * {@code nsh/mobile}'s {@code Switch} on Android: the framework's switch, in
 * the system's look and with its own animation. The app owns the value: the
 * user's choice is reported with {@code onChange}, and the switch shows the
 * value the app passes back.
 */
final class SwitchComponent implements NshComponent {
    /** Its name ({@code nsh_core::platform::SWITCH_COMPONENT}). */
    static final String NAME = "nsh/switch";

    private final Switch view;
    private final NshComponentContext events;

    SwitchComponent(Context context, NshComponentContext events) {
        this.events = events;
        this.view = new Switch(context);
        this.view.setOnCheckedChangeListener((button, checked) -> events.emit("change", checked));
    }

    @Override
    public View view() {
        return view;
    }

    @Override
    public void setProp(String name, Object value) {
        // `Switch`'s own props arrive as `$checked` and `$disabled`.
        if (name.startsWith("$")) name = name.substring(1);
        switch (name) {
            case "checked":
                boolean on = value instanceof Boolean && (Boolean) value;
                if (view.isChecked() != on) view.setChecked(on);
                break;
            case "disabled":
                view.setEnabled(!(value instanceof Boolean && (Boolean) value));
                break;
            default:
                break;
        }
    }
}
