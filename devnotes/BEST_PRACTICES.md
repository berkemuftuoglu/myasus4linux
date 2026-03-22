# myasus4linux -- Technology Stack Best Practices

This document defines the authoritative best practices for every technology in the
myasus4linux stack. All code must follow these patterns.

---

## 1. Rust Best Practices for System Tools

### Edition and MSRV

- Use **Rust 2024 edition** (stabilized in Rust 1.85.0, February 2025).
- Set `rust-version` in `Cargo.toml` to the minimum Rust version that ships with your
  target distros (e.g., Fedora, Arch, Ubuntu). For a GTK4/libadwaita app this is
  typically tied to the GTK-rs crate requirements.
- The 2024 edition enables the MSRV-aware dependency resolver by default (`resolver = "3"`).

```toml
[package]
name = "myasus4linux"
version = "0.1.0"
edition = "2024"
rust-version = "1.85"

[package.metadata]
# Resolver v3 is implicit with edition 2024
```

### Error Handling: thiserror + anyhow

Use **both** crates with clear separation:

- `thiserror` -- for defining structured error types in internal modules/libraries
  where callers need to match on specific failure modes.
- `anyhow` -- at the application boundary (main, top-level command handlers) where
  errors are displayed to the user or logged.

```rust
// src/backend/error.rs -- structured errors for internal modules
use thiserror::Error;

#[derive(Debug, Error)]
pub enum BackendError {
    #[error("failed to read sysfs attribute {path}")]
    SysfsRead {
        path: String,
        #[source]
        source: std::io::Error,
    },
    #[error("unsupported charge threshold value {0} (expected 0-100)")]
    InvalidThreshold(u8),
    #[error("D-Bus call to helper failed")]
    Dbus(#[from] zbus::Error),
}
```

```rust
// src/main.rs -- application boundary uses anyhow
use anyhow::{Context, Result};

fn main() -> Result<glib::ExitCode> {
    let res = gio::Resource::load(config::resources_file())
        .context("Could not load gresource file")?;
    gio::resources_register(&res);
    // ...
}
```

**Error message conventions:**
- Lowercase, no trailing period: `"failed to read sysfs attribute"`
- Describe only the current layer; do not re-format the source error.
- Use `.context()` / `.with_context()` to add human-readable information at each call site.

### Project Structure

```
myasus4linux/
├── Cargo.toml
├── Cargo.lock              # Committed for applications
├── meson.build
├── meson.options
├── build-aux/
│   └── flatpak/
│       └── io.github.user.MyAsus4Linux.json
├── data/
│   ├── meson.build
│   ├── icons/
│   │   ├── meson.build
│   │   └── io.github.user.MyAsus4Linux.svg
│   ├── resources/
│   │   ├── meson.build
│   │   ├── resources.gresource.xml
│   │   └── ui/
│   │       ├── window.ui
│   │       └── preferences.ui
│   ├── io.github.user.MyAsus4Linux.desktop.in
│   ├── io.github.user.MyAsus4Linux.gschema.xml.in
│   └── io.github.user.MyAsus4Linux.metainfo.xml.in
├── polkit/
│   └── io.github.user.MyAsus4Linux.Helper.policy
├── src/
│   ├── meson.build
│   ├── main.rs
│   ├── config.rs
│   ├── app.rs              # Relm4 App component
│   ├── backend/
│   │   ├── mod.rs
│   │   ├── error.rs
│   │   ├── sysfs.rs        # sysfs read/write helpers
│   │   ├── dbus_client.rs  # D-Bus client to privileged helper
│   │   └── models.rs       # Data models (ChargeThreshold, etc.)
│   ├── components/
│   │   ├── mod.rs
│   │   ├── battery_page.rs
│   │   ├── performance_page.rs
│   │   └── keyboard_page.rs
│   └── helper/
│       ├── main.rs          # Polkit-protected sysfs writer daemon
│       └── dbus_service.rs
└── tests/
```

### Clippy Lints

Enable these in `Cargo.toml` (not in source files, so they apply workspace-wide):

```toml
[lints.clippy]
# Enable the pedantic group, then selectively allow noisy lints
pedantic = { level = "warn", priority = -1 }

# Allow these pedantic lints that conflict with GTK-rs patterns
module_name_repetitions = "allow"
must_use_candidate = "allow"
missing_errors_doc = "allow"
missing_panics_doc = "allow"
wildcard_imports = "allow"        # gtk::prelude::* is idiomatic

# Specific useful lints from restriction group (cherry-pick only)
clone_on_ref_ptr = "warn"         # Catch accidental GObject clones
dbg_macro = "warn"
print_stderr = "warn"
print_stdout = "warn"             # Use g_warning!/g_info! or tracing instead
todo = "warn"
unimplemented = "warn"
unwrap_used = "warn"              # Force explicit error handling
```

```toml
[lints.rust]
unsafe_code = "deny"
```

### Key Rust Conventions

- **Commit `Cargo.lock`** -- this is an application, not a library.
- **No `unsafe` code** in the GUI layer. If unsafe is needed for sysfs, isolate it in
  the helper binary.
- Use `#[must_use]` on functions returning `Result`.
- Prefer `impl Into<String>` over `String` for function parameters.
- Use `Option<T>` instead of sentinel values.

---

## 2. GTK4-rs Best Practices

### GObject Subclassing Pattern

Every custom widget follows this two-file pattern:

```rust
// src/components/battery_row/imp.rs
use std::cell::Cell;
use gtk::glib;
use gtk::subclass::prelude::*;

#[derive(Default, gtk::CompositeTemplate)]
#[template(resource = "/io/github/user/MyAsus4Linux/ui/battery_row.ui")]
pub struct BatteryRow {
    #[template_child]
    pub threshold_spin: TemplateChild<gtk::SpinButton>,
    pub current_value: Cell<u8>,
}

#[glib::object_subclass]
impl ObjectSubclass for BatteryRow {
    const NAME: &'static str = "MyAsus4LinuxBatteryRow";
    type Type = super::BatteryRow;
    type ParentType = adw::ActionRow;

    fn class_init(klass: &mut Self::Class) {
        klass.bind_template();
    }

    fn instance_init(obj: &glib::subclass::InitializingObject<Self>) {
        obj.init_template();
    }
}

impl ObjectImpl for BatteryRow {}
impl WidgetImpl for BatteryRow {}
impl ListBoxRowImpl for BatteryRow {}
impl PreferencesRowImpl for BatteryRow {}
impl ActionRowImpl for BatteryRow {}
```

```rust
// src/components/battery_row/mod.rs
mod imp;

use gtk::glib;

glib::wrapper! {
    pub struct BatteryRow(ObjectSubclass<imp::BatteryRow>)
        @extends adw::ActionRow, adw::PreferencesRow, gtk::ListBoxRow,
                 gtk::Widget,
        @implements gtk::Accessible, gtk::Buildable, gtk::Actionable;
}

impl BatteryRow {
    pub fn new() -> Self {
        glib::Object::builder().build()
    }
}
```

**Rules:**
- `NAME` must be globally unique -- prefix with your project name.
- You must implement **all** ancestor `*Impl` traits even if empty.
- List all ancestors except `GObject` and `GInitiallyUnowned` in `@extends`.
- List all implemented interfaces in `@implements`.

### Signal Handling with `glib::clone!`

Always use `glib::clone!` to manage reference lifetimes in signal closures:

```rust
button.connect_clicked(glib::clone!(
    #[weak]
    label,
    #[weak]
    self as this,
    move |_| {
        label.set_text("Clicked!");
        this.do_something();
    }
));
```

**Rules:**
- Use `#[weak]` for any GObject reference inside signal closures. This prevents
  reference cycles and ensures cleanup when the widget hierarchy is destroyed.
- Use `#[strong]` only when you need to guarantee the object lives at least as long
  as the closure (rare -- usually for one-shot closures).
- Never capture `self` strongly in a signal handler -- always `#[weak] self as this`.

### Threading Model -- CRITICAL

**GTK is single-threaded. All GTK API calls MUST happen on the main thread.**

```rust
// CORRECT: spawn work on a background thread, send results to main thread
let (sender, receiver) = async_channel::bounded(1);

std::thread::spawn(move || {
    let result = expensive_sysfs_read();
    sender.send_blocking(result).unwrap();
});

glib::spawn_future_local(async move {
    while let Ok(result) = receiver.recv().await {
        // Safe: this runs on the main thread
        label.set_text(&result);
    }
});
```

```rust
// WRONG: calling GTK from a background thread
std::thread::spawn(move || {
    label.set_text("crash"); // UNDEFINED BEHAVIOR
});
```

**Alternative with `gio::spawn_blocking`:**

```rust
glib::spawn_future_local(async move {
    let result = gio::spawn_blocking(move || {
        std::fs::read_to_string("/sys/class/power_supply/BAT0/charge_control_end_threshold")
    }).await.unwrap();
    label.set_text(&result.unwrap());
});
```

### Memory Management

- GTK objects use **reference counting**, not Rust ownership.
- Cloning a GTK object (e.g., `button.clone()`) increments the reference count -- it
  does NOT deep-copy the widget.
- Use `Cell<T>` for `Copy` types (integers, booleans) in widget state.
- Use `RefCell<T>` for non-`Copy` types (Strings, Vecs) in widget state.
- Use `Rc<RefCell<T>>` when state is shared across multiple closures.
- **Never** create reference cycles between GObjects. Use weak references to break cycles.

---

## 3. libadwaita Best Practices

### Widget Selection Guide

| Widget | Use When |
|---|---|
| `adw::Application` | Always -- replaces `gtk::Application` |
| `adw::ApplicationWindow` | Main window -- replaces `gtk::ApplicationWindow` |
| `adw::NavigationSplitView` | Sidebar + content layout (master-detail) |
| `adw::ViewSwitcher` / `adw::ViewStack` | 2-5 top-level pages of equal importance |
| `adw::PreferencesWindow` | Dedicated settings/preferences window |
| `adw::PreferencesPage` | A page within preferences (or main UI for settings apps) |
| `adw::PreferencesGroup` | Grouped section with title and description |
| `adw::ActionRow` | General row with title, subtitle, and suffix widgets |
| `adw::SwitchRow` | Boolean on/off toggle (replaces ActionRow + Switch) |
| `adw::SpinRow` | Numeric value with increment/decrement (e.g., charge threshold) |
| `adw::ComboRow` | Selection from a list of predefined options (e.g., fan profile) |
| `adw::EntryRow` | Free-form text input with title |
| `adw::ExpanderRow` | Collapsible section with child rows |
| `adw::HeaderBar` | Replaces `gtk::HeaderBar` -- auto-handles back buttons |
| `adw::ToolbarView` | Wraps content with top/bottom bars |
| `adw::StatusPage` | Empty/error/loading states |
| `adw::ToastOverlay` + `adw::Toast` | Transient notifications |
| `adw::Banner` | Persistent notifications (e.g., "Unsaved changes") |

### For myasus4linux specifically:

```
Main Window (adw::ApplicationWindow)
└── adw::ToolbarView
    ├── [top] adw::HeaderBar with adw::ViewSwitcher
    └── [content] adw::ViewStack
        ├── "Battery" page (adw::PreferencesPage)
        │   └── adw::PreferencesGroup "Charge Limit"
        │       ├── adw::SpinRow "End Threshold" (60-100)
        │       └── adw::SpinRow "Start Threshold" (40-95)
        ├── "Performance" page (adw::PreferencesPage)
        │   └── adw::PreferencesGroup "Power Profile"
        │       ├── adw::ComboRow "Profile" [Performance, Balanced, Quiet]
        │       └── adw::SwitchRow "Panel Overdrive"
        └── "Keyboard" page (adw::PreferencesPage)
            └── adw::PreferencesGroup "Backlight"
                ├── adw::ComboRow "Mode" [Static, Breathing, Color Cycle]
                └── adw::SpinRow "Brightness" (0-3)
```

### Styling Conventions

- Use the `boxed-list` style class on `gtk::ListBox` for preference-style lists:
  ```rust
  list_box.add_css_class("boxed-list");
  ```
- **Never use boxed-list with recycling list views** (`gtk::ListView`) -- only for
  small, static lists in `gtk::ListBox`.
- Do not add custom CSS unless absolutely necessary. libadwaita's built-in styles
  handle dark/light themes automatically.

### Dark/Light Theme Handling

- **Do nothing.** libadwaita follows the system color scheme automatically.
- If you must read the theme for conditional logic:
  ```rust
  let style_manager = adw::StyleManager::default();
  let is_dark = style_manager.is_dark();
  style_manager.connect_dark_notify(|manager| {
      let is_dark = manager.is_dark();
      // React to theme change
  });
  ```
- For custom colors, use named colors from the Adwaita palette (CSS variables like
  `@accent_color`, `@warning_color`), never hardcoded hex values.
- SVG icons must work in both themes -- use symbolic icons with `*-symbolic.svg` naming.

### Adaptive Layout

- Use `adw::NavigationSplitView` -- it automatically collapses to a single pane on
  narrow screens.
- Use `adw::ViewSwitcher` in the header bar and set `adw::ViewSwitcherBar` at the bottom
  for mobile-width fallback. Connect them via `adw::ViewStack`:
  ```xml
  <object class="AdwViewSwitcher" id="switcher_title">
    <property name="stack">stack</property>
    <property name="policy">wide</property>
  </object>
  ```
- Use `adw::Breakpoint` for responsive layouts that change at specific widths.

---

## 4. Relm4 Best Practices

### Component Architecture

Each logical UI section is a **Component** with its own model, messages, and view:

```rust
use relm4::prelude::*;

// Model -- holds component state
struct BatteryPage {
    charge_end: u8,
    charge_start: u8,
    loading: bool,
}

// Messages
#[derive(Debug)]
enum BatteryInput {
    SetChargeEnd(u8),
    SetChargeStart(u8),
    LoadValues,
    ValuesLoaded { start: u8, end: u8 },
}

#[derive(Debug)]
enum BatteryOutput {
    Error(String),
}

#[relm4::component]
impl SimpleComponent for BatteryPage {
    type Init = ();
    type Input = BatteryInput;
    type Output = BatteryOutput;

    view! {
        adw::PreferencesPage {
            set_title: "Battery",
            set_icon_name: Some("battery-symbolic"),

            adw::PreferencesGroup {
                set_title: "Charge Limit",

                adw::SpinRow {
                    set_title: "End Threshold",
                    set_subtitle: "Stop charging at this percentage",
                    #[watch]
                    set_value: model.charge_end as f64,
                    set_adjustment: Some(&gtk::Adjustment::new(
                        80.0, 60.0, 100.0, 1.0, 5.0, 0.0
                    )),
                    connect_changed[sender] => move |row| {
                        sender.input(BatteryInput::SetChargeEnd(row.value() as u8));
                    },
                },
            },
        }
    }

    fn init(
        _init: Self::Init,
        root: Self::Root,
        sender: ComponentSender<Self>,
    ) -> ComponentParts<Self> {
        let model = BatteryPage {
            charge_end: 100,
            charge_start: 0,
            loading: true,
        };
        let widgets = view_output!();
        sender.input(BatteryInput::LoadValues);
        ComponentParts { model, widgets }
    }

    fn update(&mut self, msg: Self::Input, sender: ComponentSender<Self>) {
        match msg {
            BatteryInput::SetChargeEnd(val) => {
                self.charge_end = val;
                // Write to sysfs via D-Bus helper
            }
            BatteryInput::SetChargeStart(val) => {
                self.charge_start = val;
            }
            BatteryInput::LoadValues => {
                // Spawn async read
                sender.oneshot_command(async {
                    // read from sysfs
                    BatteryInput::ValuesLoaded { start: 40, end: 80 }
                });
            }
            BatteryInput::ValuesLoaded { start, end } => {
                self.charge_start = start;
                self.charge_end = end;
                self.loading = false;
            }
        }
    }
}
```

### Message Passing Patterns

1. **Parent-to-child:** Call `controller.sender().send(ChildInput::Msg)`.
2. **Child-to-parent:** Use `.forward()` when building the child controller:
   ```rust
   let battery_page = BatteryPage::builder()
       .launch(())
       .forward(sender.input_sender(), |msg| match msg {
           BatteryOutput::Error(e) => AppInput::ShowError(e),
       });
   ```
3. **Between siblings:** Route through the parent. Sibling A emits output, parent
   receives it, parent sends input to sibling B.
4. **Async results:** Use `sender.oneshot_command()` for fire-and-forget async or
   `sender.command()` for streaming results. The result arrives as `CommandOutput`.

### When to Use Relm4 vs Raw gtk4-rs

| Use Relm4 when... | Use raw gtk4-rs when... |
|---|---|
| Building page-level components | Writing custom GObject widgets |
| Managing complex state + UI updates | Implementing reusable library widgets |
| Handling async operations | Overriding virtual methods (draw, measure) |
| Building the app skeleton | Creating composite templates for .ui files |

**Rule of thumb:** Use Relm4 for the application layer (pages, dialogs, app shell) and
raw gtk4-rs for reusable widget implementations that need GObject subclassing.

### Factory Pattern for Dynamic Lists

Use `FactoryVecDeque` for lists of dynamic items:

```rust
#[relm4::factory]
impl FactoryComponent for FanProfile {
    type Init = ProfileData;
    type Input = ProfileMsg;
    type Output = ProfileOutput;
    type CommandOutput = ();
    type ParentWidget = gtk::ListBox;

    view! {
        adw::ActionRow {
            set_title: &self.name,
            #[watch]
            set_subtitle: &format!("{}W TDP", self.tdp),
        }
    }

    fn init_model(init: Self::Init, _index: &DynamicIndex, _sender: FactorySender<Self>) -> Self {
        FanProfile { name: init.name, tdp: init.tdp }
    }
}
```

**Important:** Always use `FactoryVecDeque::guard()` for mutations -- the RAII guard
batches UI updates and auto-renders on drop:
```rust
let mut guard = self.profiles.guard();
guard.push_back(new_profile);
// UI updates when guard is dropped
```

### Common Pitfalls

- **Infinite loops:** If `#[watch]` triggers a signal that sends the same message,
  use `#[block_signal]` on the widget property to prevent recursion.
- **Dropped controllers:** Store `Controller<T>` in the parent model. Dropping it
  destroys the runtime and all message receivers. Call `.detach_runtime()` if the
  component must outlive its controller.
- **Private types:** Use `#[component(pub)]` or `#[factory(pub)]` if the model is public.

---

## 5. Meson Build System Best Practices

### Root `meson.build`

```meson
project(
    'myasus4linux',
    'rust',
    version: '0.1.0',
    meson_version: '>= 1.4',
    license: 'GPL-3.0-or-later',
)

gnome = import('gnome')

base_id = 'io.github.user.MyAsus4Linux'
is_devel = get_option('profile') == 'development'

if is_devel
    profile = 'Devel'
    application_id = '@0@.@1@'.format(base_id, profile)
else
    profile = ''
    application_id = base_id
endif

bindir = get_option('prefix') / get_option('bindir')
datadir = get_option('prefix') / get_option('datadir')
pkgdatadir = datadir / meson.project_name()
subdir('data')
subdir('src')

gnome.post_install(
    gtk_update_icon_cache: true,
    glib_compile_schemas: true,
    update_desktop_database: true,
)
```

### Build Options (`meson.options`)

```meson
option(
    'profile',
    type: 'combo',
    choices: ['default', 'development'],
    value: 'default',
    description: 'Build profile for the application.',
)
```

### Cargo Integration (`src/meson.build`)

```meson
cargo = find_program('cargo')
cargo_options = ['--manifest-path', meson.project_source_root() / 'Cargo.toml']
cargo_options += ['--target-dir', meson.project_build_root() / 'target']

if not is_devel
    cargo_options += ['--release']
    rust_target = 'release'
else
    rust_target = 'debug'
endif

custom_target(
    'cargo-build',
    build_by_default: true,
    build_always_stale: true,
    output: meson.project_name(),
    console: true,
    install: true,
    install_dir: bindir,
    depends: resources,
    env: {
        'CARGO_HOME': meson.project_build_root() / 'cargo-home',
        'APP_ID': application_id,
        'RESOURCES_FILE': pkgdatadir / 'resources.gresource',
    },
    command: [
        cargo, 'build', cargo_options,
        '&&', 'cp',
        meson.project_build_root() / 'target' / rust_target / meson.project_name(),
        '@OUTPUT@',
    ],
)
```

### Config Module (`src/config.rs`)

```rust
pub const APP_ID: &str = match option_env!("APP_ID") {
    Some(v) => v,
    None => "io.github.user.MyAsus4Linux.Devel",
};
pub const RESOURCES_FILE: &str = match option_env!("RESOURCES_FILE") {
    Some(v) => v,
    None => "data/resources/resources.gresource",
};
```

### GResource Compilation (`data/resources/meson.build`)

```meson
resources = gnome.compile_resources(
    'resources',
    'resources.gresource.xml',
    gresource_bundle: true,
    source_dir: meson.current_source_dir(),
    install: true,
    install_dir: pkgdatadir,
)
```

### Desktop File + GSchema + Icons (`data/meson.build`)

```meson
subdir('resources')

if host_machine.system() == 'linux'
    subdir('icons')

    desktop_conf = configuration_data()
    desktop_conf.set('APP_ID', application_id)
    configure_file(
        input: '@0@.desktop.in.in'.format(base_id),
        output: '@0@.desktop'.format(application_id),
        configuration: desktop_conf,
        install: true,
        install_dir: datadir / 'applications',
    )

    gschema_conf = configuration_data()
    gschema_conf.set('APP_ID', application_id)
    configure_file(
        input: '@0@.gschema.xml.in'.format(base_id),
        output: '@0@.gschema.xml'.format(application_id),
        configuration: gschema_conf,
        install: true,
        install_dir: datadir / 'glib-2.0' / 'schemas',
    )
endif
```

---

## 6. Polkit Best Practices

### Architecture for sysfs Writes

**Never run the GUI as root.** Instead, use a privileged D-Bus helper:

```
User GUI (unprivileged)
    │
    ├── reads sysfs directly (no root needed for most reads)
    │
    └── writes via D-Bus ──► Helper daemon (runs as root or via polkit)
                                 │
                                 ├── validates input
                                 ├── checks polkit authorization
                                 └── writes to sysfs
```

### Policy File (`polkit/io.github.user.MyAsus4Linux.Helper.policy`)

```xml
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE policyconfig PUBLIC
 "-//freedesktop//DTD PolicyKit Policy Configuration 1.0//EN"
 "http://www.freedesktop.org/software/polkit/policyconfig-1.dtd">
<policyconfig>

  <vendor>myasus4linux</vendor>
  <vendor_url>https://github.com/user/myasus4linux</vendor_url>

  <!-- Fine-grained action for battery charge threshold writes -->
  <action id="io.github.user.MyAsus4Linux.Helper.SetChargeThreshold">
    <description>Set battery charge threshold</description>
    <message>Authentication is required to change the battery charge threshold</message>
    <icon_name>battery-symbolic</icon_name>
    <defaults>
      <allow_any>auth_admin</allow_any>
      <allow_inactive>auth_admin</allow_inactive>
      <allow_active>auth_admin_keep</allow_active>
    </defaults>
  </action>

  <!-- Separate action for performance profile writes -->
  <action id="io.github.user.MyAsus4Linux.Helper.SetPerformanceProfile">
    <description>Set ASUS performance profile</description>
    <message>Authentication is required to change the performance profile</message>
    <icon_name>speedometer-symbolic</icon_name>
    <defaults>
      <allow_any>auth_admin</allow_any>
      <allow_inactive>auth_admin</allow_inactive>
      <allow_active>auth_admin_keep</allow_active>
    </defaults>
  </action>

</policyconfig>
```

### Least Privilege Principles

1. **One action per operation type.** Do NOT create a single "do everything" action.
   Create separate actions for battery, performance, keyboard, etc.
2. **Use `auth_admin_keep` for active sessions** -- the user authenticates once, and
   the authorization is cached for ~5 minutes. This avoids repeated password prompts
   while maintaining security.
3. **Validate all input in the helper.** Never trust data from the GUI:
   ```rust
   fn set_charge_threshold(value: u8) -> Result<(), HelperError> {
       if !(20..=100).contains(&value) {
           return Err(HelperError::InvalidValue("threshold must be 20-100"));
       }
       let path = "/sys/class/power_supply/BAT0/charge_control_end_threshold";
       // Validate path is what we expect -- no path traversal
       std::fs::write(path, value.to_string())
           .map_err(HelperError::SysfsWrite)?;
       Ok(())
   }
   ```
4. **Hardcode sysfs paths** in the helper. Never accept arbitrary paths from the client.
5. **Drop privileges** if the helper daemon stays running -- use a dedicated system user.

### D-Bus System Service for the Helper

Install a D-Bus system service file at
`/usr/share/dbus-1/system-services/io.github.user.MyAsus4Linux.Helper.service`:

```ini
[D-BUS Service]
Name=io.github.user.MyAsus4Linux.Helper
Exec=/usr/libexec/myasus4linux-helper
User=root
SystemdService=myasus4linux-helper.service
```

And a D-Bus system config at
`/usr/share/dbus-1/system.d/io.github.user.MyAsus4Linux.Helper.conf`:

```xml
<!DOCTYPE busconfig PUBLIC
 "-//freedesktop//DTD D-BUS Bus Configuration 1.0//EN"
 "http://www.freedesktop.org/standards/dbus/1.0/busconfig.dtd">
<busconfig>
  <policy user="root">
    <allow own="io.github.user.MyAsus4Linux.Helper"/>
    <allow send_destination="io.github.user.MyAsus4Linux.Helper"/>
  </policy>
  <policy context="default">
    <allow send_destination="io.github.user.MyAsus4Linux.Helper"
           send_interface="io.github.user.MyAsus4Linux.Helper"/>
  </policy>
</busconfig>
```

---

## 7. Flatpak Packaging Best Practices

### Manifest (`build-aux/flatpak/io.github.user.MyAsus4Linux.json`)

```json
{
    "app-id": "io.github.user.MyAsus4Linux",
    "runtime": "org.gnome.Platform",
    "runtime-version": "48",
    "sdk": "org.gnome.Sdk",
    "sdk-extensions": [
        "org.freedesktop.Sdk.Extension.rust-stable"
    ],
    "command": "myasus4linux",
    "finish-args": [
        "--share=ipc",
        "--socket=fallback-x11",
        "--socket=wayland",
        "--device=dri",
        "--system-talk-name=io.github.user.MyAsus4Linux.Helper",
        "--talk-name=org.freedesktop.Notifications"
    ],
    "build-options": {
        "append-path": "/usr/lib/sdk/rust-stable/bin",
        "build-args": [
            "--share=network"
        ],
        "env": {
            "CARGO_HOME": "/run/build/myasus4linux/cargo",
            "CARGO_REGISTRIES_CRATES_IO_PROTOCOL": "sparse"
        }
    },
    "cleanup": [
        "/include",
        "/lib/pkgconfig",
        "/share/man",
        "*.la",
        "*.a"
    ],
    "modules": [
        {
            "name": "myasus4linux",
            "builddir": true,
            "buildsystem": "meson",
            "config-opts": [
                "-Dprofile=default"
            ],
            "sources": [
                {
                    "type": "dir",
                    "path": "../../"
                }
            ]
        }
    ]
}
```

### Runtime Selection

- Use `org.gnome.Platform` / `org.gnome.Sdk` for libadwaita apps -- it includes GTK4
  and libadwaita.
- Use the latest stable runtime version (currently `47`).
- The Rust SDK extension is under `org.freedesktop.Sdk.Extension.rust-stable` (even
  when using the GNOME SDK, the extension comes from freedesktop).

### Permissions Model

Apply the **principle of least privilege**:

- `--socket=wayland` + `--socket=fallback-x11` -- display access.
- `--share=ipc` -- required for X11 shared memory.
- `--device=dri` -- GPU acceleration.
- `--system-talk-name=io.github.user.MyAsus4Linux.Helper` -- allow talking to the
  privileged helper only. Do NOT use `--socket=system-bus` (grants access to ALL
  system D-Bus services).
- Do NOT request `--filesystem=host` or `--filesystem=home`. The app reads sysfs via
  the helper, not directly.

### Flatpak-Specific Notes

- The privileged helper daemon runs **outside** the Flatpak sandbox (installed
  system-wide via RPM/DEB). Flatpak only packages the GUI.
- For Flathub submission, vendor all Cargo dependencies using
  `flatpak-cargo-generator.py` to generate a cargo sources manifest.
- Network access is only needed at build time (`--share=network` in `build-args`),
  not at runtime.

---

## 8. GNOME HIG (Human Interface Guidelines) for App Design

### App Identity

- Use a **reverse-DNS app ID**: `io.github.user.MyAsus4Linux`
- Provide a symbolic icon (`*-symbolic.svg`) for the header bar and a full-color icon
  for the app grid.
- Include AppStream metainfo for software center listings.

### Navigation Pattern

For myasus4linux (a settings/utility app with 3-5 pages), use the **View Switcher** pattern:

- Place `adw::ViewSwitcher` in the header bar (wide mode).
- Add `adw::ViewSwitcherBar` at the bottom (narrow/mobile fallback).
- Each page is a child of `adw::ViewStack`.
- Pages are of **equal importance** -- no hierarchy needed.

**Do NOT use:**
- Tab bars (not a GNOME pattern).
- Sidebar navigation (overkill for < 6 pages).
- Deep navigation hierarchies.

### Header Bar

- Use `adw::HeaderBar` -- never `gtk::HeaderBar`.
- Place the app menu (primary menu) on the trailing end (right in LTR).
- Primary menu contains: Keyboard Shortcuts, About, and optionally Preferences.
- **No** custom title widgets unless using a view switcher.
- Back buttons are handled automatically by `adw::NavigationView`.

### Preferences Page Layout

Each page follows this structure:

```
PreferencesPage (title + icon for ViewStack)
├── PreferencesGroup (title: "Section Name", description: optional)
│   ├── SwitchRow / SpinRow / ComboRow / ActionRow
│   ├── SwitchRow / SpinRow / ComboRow / ActionRow
│   └── ...
├── PreferencesGroup (title: "Another Section")
│   └── ...
└── ...
```

**Rules:**
- Group related settings into `PreferencesGroup`.
- Use descriptive group titles and optional group descriptions.
- Each row gets a clear title and an optional subtitle explaining the setting.
- Use the most specific row widget (`SwitchRow` for booleans, `SpinRow` for numbers,
  `ComboRow` for enums) -- do not use `ActionRow` when a specialized widget exists.
- Keep groups to 3-7 items. Split into multiple groups if needed.

### Feedback Patterns

- Use `adw::Toast` for transient success/info messages ("Profile changed to Quiet").
- Use `adw::Banner` for persistent warnings ("Running on AC power required").
- Use `adw::StatusPage` for empty/error/loading states.
- Never use modal dialogs for confirmations unless the action is destructive and irreversible.

### General Design Principles

- **Progressive disclosure:** Show essential controls first; advanced options behind
  expander rows or a separate preferences window.
- **Consistency:** Follow GNOME widget patterns -- do not invent custom controls when
  standard ones exist.
- **Keyboard navigation:** All controls must be keyboard-accessible. Test with Tab/Shift+Tab.
- **Accessibility:** Set appropriate `accessible-role` and labels on all interactive widgets.
- Use `Ctrl+Shift+D` during development to open GTK Inspector for debugging layout and CSS.

---

## Quick Reference: Cargo.toml Dependencies

```toml
[dependencies]
gtk = { version = "0.11", package = "gtk4", features = ["v4_18"] }
adw = { version = "0.9", package = "libadwaita", features = ["v1_8"] }
relm4 = { version = "0.10", features = ["libadwaita"] }
glib = "0.22"
gio = "0.22"
anyhow = "1"
thiserror = "2"
zbus = "5"               # D-Bus client for polkit helper
async-channel = "2"
tracing = "0.1"
tracing-subscriber = "0.3"

[build-dependencies]
glib-build-tools = "0.22" # Only needed when NOT using Meson for resources
```

**Note:** Pin version features to match your target runtime. If targeting GNOME 48
runtime, use GTK 4.18 (`v4_18`) and libadwaita 1.8 (`v1_8`).
