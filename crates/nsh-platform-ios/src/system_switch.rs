//! `nsh/mobile`'s `Switch` on iOS: a `UISwitch`, the system's on / off
//! control, which draws, animates and reports its own presses.

use std::rc::Rc;

use nsh_core::JsonValue;
use objc2::{
    DefinedClass, MainThreadMarker, MainThreadOnly, define_class, msg_send, rc::Retained, sel,
};
use objc2_foundation::{NSObject, NSObjectProtocol};
use objc2_ui_kit::{UIControlEvents, UISwitch, UIView};

use crate::component::{ComponentContext, ComponentFactory, NativeComponent};

struct TargetIvars {
    context: ComponentContext,
}

define_class!(
    /// Reports the value the user chose, as `onChange(on)`.
    #[unsafe(super(NSObject))]
    #[thread_kind = MainThreadOnly]
    #[ivars = TargetIvars]
    struct SwitchTarget;

    unsafe impl NSObjectProtocol for SwitchTarget {}

    impl SwitchTarget {
        #[unsafe(method(valueChanged:))]
        fn value_changed(&self, sender: &UISwitch) {
            self.ivars()
                .context
                .emit("change", serde_json::json!(sender.isOn()));
        }
    }
);

impl SwitchTarget {
    fn new(mtm: MainThreadMarker, context: ComponentContext) -> Retained<Self> {
        let this = Self::alloc(mtm).set_ivars(TargetIvars { context });
        // SAFETY: NSObject's designated initializer.
        unsafe { msg_send![super(this), init] }
    }
}

/// The switch of one `Switch` element.
pub(crate) struct SystemSwitch {
    control: Retained<UISwitch>,
    /// The switch reports presses to it; it lives as long as the switch.
    _target: Retained<SwitchTarget>,
}

impl NativeComponent for SystemSwitch {
    fn view(&self) -> &UIView {
        &self.control
    }

    fn set_prop(&mut self, name: &str, value: Option<&JsonValue>) {
        // `Switch`'s own props arrive as `$checked` and `$disabled`.
        match (name.strip_prefix('$').unwrap_or(name), value) {
            ("checked", value) => {
                let on = value.and_then(JsonValue::as_bool).unwrap_or(false);
                if self.control.isOn() != on {
                    // The system's own slide, as a press has it.
                    self.control.setOn_animated(on, true);
                }
            }
            // UIKit dims a disabled control by itself.
            ("disabled", value) => {
                let disabled = value.and_then(JsonValue::as_bool).unwrap_or(false);
                self.control.setEnabled(!disabled);
            }
            _ => {}
        }
    }
}

/// Makes the switch of each `Switch` element (`nsh_core::platform::SWITCH_COMPONENT`).
pub(crate) fn factory() -> ComponentFactory {
    Rc::new(|context: ComponentContext| {
        let mtm = MainThreadMarker::new().expect("a switch is made on the main thread");
        let control = UISwitch::new(mtm);
        // Its size is the system's: the app sizes it only where its style says so.
        let size = control.intrinsicContentSize();
        context.set_size(size.width as f32, size.height as f32);
        let target = SwitchTarget::new(mtm, context);
        // SAFETY: `valueChanged:` is implemented by `SwitchTarget` above.
        unsafe {
            control.addTarget_action_forControlEvents(
                Some(&target),
                sel!(valueChanged:),
                UIControlEvents::ValueChanged,
            )
        };
        Box::new(SystemSwitch {
            control,
            _target: target,
        })
    })
}
