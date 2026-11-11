#!/bin/sh
# Prepare this machine to move the NVIDIA card between the desktop and VMs
# with the gpu-vfio-control service. See spec/GraphicsCard.pi.
#   sudo sh contrib/gpu/setup.sh
# Each changed file is kept beside itself as <file>.pre-osmia.
set -eu

if [ "$(id -u)" -ne 0 ]; then
    echo "Run this with sudo." >&2
    exit 1
fi

keep() {
    [ -f "$1" ] && [ ! -f "$1.pre-osmia" ] && cp -p "$1" "$1.pre-osmia"
    return 0
}

# 1. The card starts on the host: stop blacklisting the nvidia modules.
#    nouveau stays blacklisted.
bl=/etc/modprobe.d/blacklist-gpu.conf
if [ -f "$bl" ] && grep -Eq '^[[:space:]]*blacklist[[:space:]]+nvidia' "$bl"; then
    keep "$bl"
    sed -i -E '/^[[:space:]]*blacklist[[:space:]]+nvidia([_a-z]*)?[[:space:]]*$/d' "$bl"
    echo "Removed the nvidia lines from $bl"
fi

# 2. vfio-pci must not claim the card at boot. It stays loaded, and the
#    service binds it when asked.
vf=/etc/modprobe.d/vfio.conf
if [ -f "$vf" ] && grep -Eq '^[[:space:]]*options[[:space:]]+vfio-pci.*ids=' "$vf"; then
    keep "$vf"
    sed -i -E 's/^([[:space:]]*options[[:space:]]+vfio-pci.*ids=.*)$/# Turned off by osmia: the card boots on the host.\n# \1/' "$vf"
    echo "Turned off the vfio-pci ids= option in $vf"
fi

# 3. The login screen's X server stays running for the whole session. Keep it
#    off the card, or it holds /dev/nvidia0 and the card cannot be handed over.
xc=/etc/X11/xorg.conf.d/21-osmia-no-dgpu.conf
if [ -d /etc/X11/xorg.conf.d ] && [ ! -f "$xc" ]; then
    cat > "$xc" <<'EOF'
# Written by osmia (contrib/gpu/setup.sh). X uses only the primary GPU, so the
# NVIDIA card stays free to be handed to a VM.
Section "ServerFlags"
    Option "AutoAddGPU" "off"
EndSection
EOF
    echo "Wrote $xc"
fi

# 4. NVreg_PreserveVideoMemoryAllocations=1 is set (by gpu-screen-recorder),
#    and the driver needs these services with it, or resume can fail.
systemctl enable nvidia-suspend.service nvidia-resume.service nvidia-hibernate.service
echo "Enabled the NVIDIA suspend, resume and hibernate services"

# 5. The modprobe files above are copied into the initramfs.
if command -v mkinitcpio >/dev/null 2>&1; then
    echo "Rebuilding the initramfs"
    mkinitcpio -P
else
    echo "Rebuild your initramfs so the modprobe changes apply at boot." >&2
fi

echo
echo "Done. Reboot, then check with: gpu-vfio-control status --human"
echo "Undo: put back the .pre-osmia copies, delete $xc, and rebuild the initramfs."
