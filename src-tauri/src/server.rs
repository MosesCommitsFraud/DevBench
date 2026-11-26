//! Setting up and updating a Linux server for DevBench. The check runs over SSH without root.
//! Installing and updating need root, so they run in DevBench's terminal where sudo asks for
//! the password itself: DevBench never sees or stores it, and you see every command run.

use serde::Serialize;

#[derive(Debug, Clone, Default, Serialize, PartialEq)]
pub struct ServerCheck {
    /// One entry per part DevBench uses, in a fixed order.
    pub parts: Vec<Part>,
    /// Package updates waiting (apt), of which security updates.
    pub updates: u32,
    pub security_updates: u32,
    /// Snaps with a newer revision.
    pub snap_updates: u32,
    /// An installed update wants a restart (/var/run/reboot-required).
    pub reboot_required: bool,
    /// The system's package manager is apt (Debian, Ubuntu); setup and updates need it.
    pub apt: bool,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct Part {
    /// "libvirt", "power" or "desktop".
    pub id: String,
    pub ok: bool,
    /// What is missing, or what was found.
    pub detail: String,
}

/// Reads everything in one go; each line is `key|value...`.
pub const CHECK: &str = r#"export LC_ALL=C
command -v apt-get >/dev/null 2>&1 && echo "apt|1"
if command -v virsh >/dev/null 2>&1; then
  if id -nG | tr ' ' '\n' | grep -qx libvirt; then
    if virsh -c qemu:///system list >/dev/null 2>&1; then echo "libvirt|1|$(virsh -c qemu:///system list --all --name | grep -c .) VMs"; else echo "libvirt|0|libvirtd is not running"; fi
  else echo "libvirt|0|user not in group libvirt"; fi
else echo "libvirt|0|not installed"; fi
if sudo -n -l 2>/dev/null | grep -q 'systemctl poweroff'; then echo "power|1|sudoers rule"; else echo "power|0|no sudoers rule"; fi
if systemctl --user is-active --quiet devbench-desktop.service 2>/dev/null; then echo "desktop|1|devbench-desktop.service, VNC :5901"
elif ss -Hltn 2>/dev/null | grep -qE '127\.0\.0\.1:59[0-9][0-9] '; then echo "desktop|1|a VNC server on localhost"
elif command -v Xtigervnc >/dev/null 2>&1; then echo "desktop|0|installed, service not running"
else echo "desktop|0|not installed"; fi
if [ -x /usr/lib/update-notifier/apt-check ]; then
  echo "updates|$(/usr/lib/update-notifier/apt-check 2>&1 | tail -n1 | tr ';' '|')"
elif command -v apt >/dev/null 2>&1; then
  echo "updates|$(apt list --upgradable 2>/dev/null | grep -c upgradable)|0"
fi
command -v snap >/dev/null 2>&1 && echo "snaps|$(snap refresh --list 2>/dev/null | tail -n +2 | grep -c .)"
[ -f /var/run/reboot-required ] && echo "reboot|1"
exit 0
"#;

pub fn parse(out: &str) -> ServerCheck {
    let mut c = ServerCheck::default();
    for line in out.lines() {
        let f: Vec<&str> = line.trim().split('|').collect();
        let n = |i: usize| f.get(i).and_then(|x| x.trim().parse::<u32>().ok()).unwrap_or(0);
        match f[0] {
            "apt" => c.apt = true,
            "libvirt" | "power" | "desktop" => c.parts.push(Part {
                id: f[0].to_string(),
                ok: f.get(1) == Some(&"1"),
                detail: f.get(2).unwrap_or(&"").trim().to_string(),
            }),
            "updates" => (c.updates, c.security_updates) = (n(1), n(2)),
            "snaps" => c.snap_updates = n(1),
            "reboot" => c.reboot_required = true,
            _ => {}
        }
    }
    c
}

/// The script for a setup step or for updating, shown and run in the terminal.
pub fn script(step: &str) -> Option<&'static str> {
    Some(match step {
        "updates" => UPDATES,
        "libvirt" => LIBVIRT,
        "power" => POWER,
        "desktop" => DESKTOP,
        _ => return None,
    })
}

const UPDATES: &str = r#"echo '== Updating packages (apt) =='
sudo apt-get update
sudo DEBIAN_FRONTEND=noninteractive apt-get full-upgrade -y
sudo apt-get autoremove -y
if command -v snap >/dev/null 2>&1; then echo '== Updating snaps =='; sudo snap refresh; fi
if [ -f /var/run/reboot-required ]; then echo '!! A restart is needed to finish the updates (DevBench: menu > Restart).'; fi
"#;

const LIBVIRT: &str = r#"echo '== Installing KVM and libvirt =='
sudo apt-get update
sudo DEBIAN_FRONTEND=noninteractive apt-get install -y qemu-kvm libvirt-daemon-system libvirt-clients virtinst
sudo systemctl enable --now libvirtd
sudo usermod -aG libvirt "$USER"
echo "Added $USER to the group libvirt. DevBench's next connection picks it up."
"#;

const POWER: &str = r#"echo '== Letting DevBench shut down and restart this server =='
SYSTEMCTL="$(command -v systemctl)"
TMP="$(mktemp)"
printf '%s ALL=(root) NOPASSWD: %s poweroff, %s reboot\n' "$USER" "$SYSTEMCTL" "$SYSTEMCTL" > "$TMP"
cat "$TMP"
sudo visudo -cf "$TMP"
sudo install -m 0440 -o root -g root "$TMP" /etc/sudoers.d/devbench
rm -f "$TMP"
sudo -n -l | grep systemctl
"#;

const DESKTOP: &str = r#"echo '== Installing a virtual Xfce desktop (TigerVNC on 127.0.0.1:5901) =='
sudo apt-get update
sudo DEBIAN_FRONTEND=noninteractive apt-get install -y --no-install-recommends xfce4 xfce4-terminal xfce4-notifyd tango-icon-theme fonts-dejavu-core dbus-x11 tigervnc-standalone-server
mkdir -p "$HOME/.local/bin" "$HOME/.config/systemd/user"
cat > "$HOME/.local/bin/devbench-desktop" <<'SCRIPT'
#!/bin/sh
# Virtual Xfce desktop on :1 = VNC port 5901, only on localhost (DevBench reaches it through SSH)
export DISPLAY=:1
Xtigervnc :1 -localhost -geometry 1920x1080 -depth 24 -SecurityTypes None -AlwaysShared -desktop "$(hostname)" &
xpid=$!
i=0
while [ ! -e /tmp/.X11-unix/X1 ] && [ $i -lt 100 ]; do sleep 0.1; i=$((i + 1)); done
dbus-run-session startxfce4
kill $xpid 2>/dev/null
SCRIPT
chmod +x "$HOME/.local/bin/devbench-desktop"
cat > "$HOME/.config/systemd/user/devbench-desktop.service" <<'UNIT'
[Unit]
Description=Virtual Xfce desktop for DevBench (VNC on 127.0.0.1:5901)

[Service]
ExecStart=%h/.local/bin/devbench-desktop
Restart=always
RestartSec=5

[Install]
WantedBy=default.target
UNIT
sudo loginctl enable-linger "$USER"
systemctl --user daemon-reload
systemctl --user enable --now devbench-desktop.service
systemctl --user --no-pager status devbench-desktop.service | head -n 5
"#;

/// The command line that runs a step in a terminal: the script stops at the first error, says
/// how it ended, and leaves a shell open to look around.
pub fn terminal_command(step: &str) -> Option<String> {
    let body = script(step)?;
    let wrapped = format!(
        "( set -e\n{body})\nif [ $? -eq 0 ]; then printf '\\n\\033[32m✓ Done.\\033[0m You can close this terminal.\\n\\n'; else printf '\\n\\033[31m✗ Stopped at an error (see above).\\033[0m\\n\\n'; fi\nexec bash -l"
    );
    Some(format!("bash -lc {}", crate::ssh::sh_quote(&wrapped)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_check() {
        let out = "apt|1\nlibvirt|1|3 VMs\npower|0|no sudoers rule\ndesktop|1|devbench-desktop.service, VNC :5901\nupdates|3|3\nsnaps|0\n";
        let c = parse(out);
        assert!(c.apt);
        assert_eq!(c.parts.len(), 3);
        assert!(c.parts[0].ok && !c.parts[1].ok && c.parts[2].ok);
        assert_eq!(c.parts[0].detail, "3 VMs");
        assert_eq!((c.updates, c.security_updates, c.snap_updates), (3, 3, 0));
        assert!(!c.reboot_required);
    }

    #[test]
    fn steps() {
        for s in ["updates", "libvirt", "power", "desktop"] {
            let cmd = terminal_command(s).unwrap();
            assert!(cmd.starts_with("bash -lc '"));
        }
        assert!(terminal_command("rm -rf /").is_none());
    }
}
