#!/bin/sh
# Grant the current user prompt-free, NON-world-writable access to the ASUS
# hardware controls. Run once as root:  sudo ./data/setup-permissions.sh
#
# Creates the "myasus" group, adds you to it, installs the udev rule, and
# applies it. Distro packages (.deb/AUR) should do the equivalent in postinst.
set -eu

GROUP=myasus
RULE=99-myasus4linux.rules
RULE_DST=/etc/udev/rules.d/$RULE

if [ "$(id -u)" -ne 0 ]; then
    echo "error: run as root (sudo $0)" >&2
    exit 1
fi

# the human who invoked sudo, not root
TARGET_USER=${SUDO_USER:-$(logname 2>/dev/null || true)}
if [ -z "${TARGET_USER:-}" ] || [ "$TARGET_USER" = root ]; then
    echo "error: could not determine the target user; run via sudo from your account" >&2
    exit 1
fi

getent group "$GROUP" >/dev/null 2>&1 || groupadd --system "$GROUP"
usermod -aG "$GROUP" "$TARGET_USER"

src_dir=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
install -Dm644 "$src_dir/$RULE" "$RULE_DST"
udevadm control --reload-rules
udevadm trigger --subsystem-match=leds --subsystem-match=platform \
    --subsystem-match=power_supply --subsystem-match=backlight

echo "Done. Log out and back in (or reboot) so '$TARGET_USER' picks up the '$GROUP' group."
