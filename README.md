# myasus4linux

An open source replacement for the ASUS MyASUS Windows app. It gives ASUS
notebook owners on Linux a native GTK4/libadwaita app to control the hardware
features that normally require the vendor's Windows-only tool.

It talks to the kernel's `asus-nb-wmi` driver through sysfs, so it works on any
ASUS laptop running a mainline kernel (ZenBook, VivoBook, ExpertBook, ProArt,
ROG, TUF). Features your model doesn't expose are detected at startup and
hidden.

## Features

- **Battery** — charge limit, charge level, health (`charge_full` vs design),
  cycle count, wear status
- **Fan** — Quiet / Balanced / Performance profile plus live CPU temperature
- **Keyboard** — backlight brightness (Off / Low / Medium / High)
- **Info** — model, CPU, RAM, kernel version

The charge limit persists across reboots via a one-shot systemd service that
re-applies your saved value at boot.

## Safeguards

Charge limit defaults to 80% and is never allowed below 40%. Fans can't be
fully disabled. Every change is reversible — nothing is written permanently.

## Build

```bash
meson setup build
meson compile -C build
```

Or for development:

```bash
cargo build
cargo run
```

Privileged sysfs writes go through polkit, so you'll be prompted to
authenticate when changing a setting.

## License

GPL-3.0-or-later. See [LICENSE](LICENSE).
