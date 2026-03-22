# myasus4linux - Product Roadmap

**Project:** MyASUS for Linux -- open source replacement for ASUS MyASUS Windows app
**Stack:** Rust + GTK4/libadwaita (Relm4)
**Target:** Any ASUS notebook running Linux (ZenBook, VivoBook, ExpertBook, ProArt, ROG, TUF)
**Kernel dependency:** `asus-nb-wmi` (mainline kernel, ships with all distros)

---

## Now (v0.1 -- Working MVP)

The goal is a single binary that launches, shows your ASUS laptop's status, and lets you control the basics.

### Must Have
- [ ] Project scaffolding (Meson + Cargo build, GTK4/libadwaita/Relm4 dependency, polkit for root writes)
- [ ] **Battery Care page**
  - Charge limit slider (60 / 80 / 100%)
  - Current charge %, voltage, current draw
  - Battery health % (charge_full vs charge_full_design)
  - Cycle count
  - Wear level with plain English status ("Good" / "Fair" / "Replace soon")
  - Warning when set to 100%: "Keeping battery at 100% reduces lifespan"
- [ ] **Fan Profile page**
  - Three-way toggle: Quiet / Balanced / Performance
  - Current mode indicator
  - Live CPU temperature reading
- [ ] **Keyboard Backlight page**
  - Slider: Off / Low / Medium / High (0-3)
- [ ] **System Info page**
  - Model name, CPU, RAM, kernel version
  - Storage info (NVMe model, capacity)
  - BIOS version
- [ ] **Hardware detection**
  - Check which sysfs paths exist on launch
  - Gracefully hide features the laptop doesn't have
  - Show "not supported on this model" for missing features

### Should Have
- [ ] Persist charge limit across reboots (write systemd service or udev rule)
- [ ] First-launch wizard ("Your battery health is X%. We recommend enabling 80% charge limit.")
- [ ] App icon and .desktop file

### Won't Have (v0.1)
- Tray icon
- Thermal dashboard
- Power monitoring
- Auto-switching profiles

---

## Next (v0.2 -- Thermal & Power)

### Must Have
- [ ] **Thermal Dashboard**
  - All detected thermal zones with live temps (1-2s polling)
  - Labels: CPU, SSD, VRM, Ambient, WiFi (auto-detect from zone type)
  - Color coding: green (<60C), yellow (60-80C), red (>80C)
- [ ] **Power Monitoring**
  - Live wattage from RAPL (package, core, uncore)
  - Battery discharge rate in watts
  - Estimated time remaining
- [ ] **WiFi/Bluetooth toggles**
  - Quick on/off via rfkill

### Should Have
- [ ] Thermal safety override (auto-switch to performance if temps critical)
- [ ] Battery low warning (suggest quiet mode when <20%)

---

## Next (v0.3 -- Smart Features)

### Must Have
- [ ] **System tray icon**
  - Quick access: fan mode toggle, battery %, current temp
  - Right-click menu with common actions
- [ ] **Auto fan profile switching**
  - Mic active → quiet mode
  - High CPU load sustained → performance mode
  - On battery → quiet mode
  - Configurable rules
- [ ] **Notifications**
  - Battery health degradation warning
  - Thermal throttling alert
  - Charge limit reminder on first plug-in

### Should Have
- [ ] Keyboard backlight auto-triggers (dim on battery, off on screen lock)
- [ ] Remember settings per power state (plugged in vs battery)

---

## Later (v0.4+ -- Community & Polish)

- [ ] Flatpak / .deb / AUR packaging
- [ ] Screen color temperature (DCTS WMI -- experimental)
- [ ] Per-model quirk database (community-contributed)
- [ ] ROG-specific features (RGB, AniMe matrix) as optional modules
- [ ] Onboarding: detect laptop model, show what features are available
- [ ] Export system report (for bug reports / support)
- [ ] CLI companion tool (for scripting / headless servers)

---

## Prioritization (RICE)

| Feature | Reach | Impact | Confidence | Effort | RICE Score |
|---|---|---|---|---|---|
| Battery charge limit | All users | 3 (massive) | 100% | 0.5 mo | 600 |
| Fan profile toggle | All users | 3 | 100% | 0.5 mo | 600 |
| Battery health display | All users | 2 | 100% | 0.25 mo | 800 |
| Keyboard backlight | Most users | 1 | 100% | 0.25 mo | 400 |
| Thermal dashboard | Power users | 2 | 80% | 1 mo | 160 |
| System tray icon | All users | 2 | 100% | 0.5 mo | 400 |
| Auto fan switching | Power users | 2 | 60% | 1.5 mo | 80 |
| Live power monitoring | Power users | 1 | 80% | 0.5 mo | 160 |
| Flatpak packaging | All users | 2 | 100% | 1 mo | 200 |

---

## Safeguards (Built-in from v0.1)

These are not features -- they are requirements baked into every release:

1. **Battery:** Default to 80% on first launch. Warn on 100%. Never allow below 40%.
2. **Thermal:** Auto-override to performance if any zone hits 90C.
3. **Fan:** Never allow complete fan disable.
4. **Power:** Suggest quiet mode when battery <20%.
5. **General:** All changes reversible. No permanent modifications. Plain English labels, not sysfs paths.
6. **Permissions:** Use polkit for privilege escalation. Never run the whole app as root.

---

## Architecture

```
myasus4linux/
  src/
    main.rs              -- Entry point, Relm4 app init
    backend/
      mod.rs             -- Backend module root
      sysfs.rs           -- Read/write sysfs files (the entire HW interface)
      detect.rs          -- Auto-detect available features
      battery.rs         -- Battery care logic + safeguards
      thermal.rs         -- Thermal zone reading + safety overrides
      fan.rs             -- Fan profile management
      keyboard.rs        -- Keyboard backlight control
      power.rs           -- RAPL power monitoring
      rfkill.rs          -- WiFi/Bluetooth toggle
    ui/
      mod.rs             -- UI module root
      app.rs             -- Main window with ViewSwitcher navigation
      battery_page.rs    -- Battery care UI (Relm4 component)
      fan_page.rs        -- Fan profile UI (Relm4 component)
      thermal_page.rs    -- Thermal dashboard UI (Relm4 component)
      keyboard_page.rs   -- Keyboard backlight UI (Relm4 component)
      info_page.rs       -- System info page (Relm4 component)
    polkit/
      policy.xml         -- Polkit policy for sysfs writes
      helper.rs          -- Privileged helper for root operations
  data/
    icons/               -- App icons
    myasus4linux.desktop -- Desktop entry
  Cargo.toml             -- Rust dependencies
  meson.build            -- Build system (orchestrates Cargo)
  LICENSE                -- GPL v3
  README.md
```

---

## Technical Decisions

| Decision | Choice | Why |
|---|---|---|
| Language | Rust | Memory safety, no runtime, strong ecosystem |
| UI Framework | GTK4 + libadwaita (Relm4) | Native GNOME look, Elm-style Rust bindings |
| Build System | Meson + Cargo | Meson for desktop integration, Cargo for Rust |
| Privilege | Polkit | Proper Linux way, no running as root |
| Config | XDG config dir | ~/.config/myasus4linux/settings.toml |
| License | GPL v3 | Matches kernel, strong copyleft |
