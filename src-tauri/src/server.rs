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
    /// Names of the packages with an update waiting (at most 60).
    pub packages: Vec<String>,
    /// DevBench's update service is running right now.
    pub updating: bool,
    /// The last update DevBench ran, from /var/log/devbench-update.log.
    pub last_update: Option<LastUpdate>,
}

#[derive(Debug, Clone, Default, Serialize, PartialEq)]
pub struct LastUpdate {
    /// Unix seconds it started, and ended (None while it runs, or if it was cut off).
    pub started: Option<u64>,
    pub finished: Option<u64>,
    /// Exit code of the first step that failed, 0 when everything went through.
    pub code: Option<i32>,
    /// Packages upgraded, summed over apt's "N upgraded" lines.
    pub upgraded: u32,
    /// apt held back updates that Ubuntu hands out in stages (phased updates).
    pub phased: bool,
    /// apt kept packages back, e.g. because they need new dependencies.
    pub kept_back: bool,
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
command -v apt >/dev/null 2>&1 && echo "pkgs|$(apt list --upgradable 2>/dev/null | awk -F/ 'NR>1 && /upgradable/ {print $1}' | head -n 60 | paste -sd' ')"
command -v snap >/dev/null 2>&1 && echo "snaps|$(snap refresh --list 2>/dev/null | tail -n +2 | grep -c .)"
[ -f /var/run/reboot-required ] && echo "reboot|1"
L=/var/log/devbench-update.log
if [ -r "$L" ]; then
  s=$(sed -n 's/^@@start //p' "$L" | tail -n1)
  d=$(sed -n 's/^@@done //p' "$L" | tail -n1)
  up=$(grep -E '^[0-9]+ upgraded' "$L" | awk '{n+=$1} END {print n+0}')
  echo "last|$s|${d% *}|${d#* }|$up|$(grep -c 'deferred due to phasing' "$L")|$(grep -c 'have been kept back' "$L")"
fi
systemctl is-active --quiet devbench-update 2>/dev/null && echo "running|1"
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
            "pkgs" => c.packages = f.get(1).map(|x| x.split_whitespace().map(String::from).collect()).unwrap_or_default(),
            "running" => c.updating = true,
            "last" => {
                let num = |i: usize| f.get(i).and_then(|x| x.trim().parse::<u64>().ok());
                c.last_update = Some(LastUpdate {
                    started: num(1),
                    code: f.get(2).and_then(|x| x.trim().parse::<i32>().ok()),
                    finished: num(3),
                    upgraded: n(4),
                    phased: n(5) > 0,
                    kept_back: n(6) > 0,
                });
            }
            "reboot" => c.reboot_required = true,
            _ => {}
        }
    }
    c
}

/// The script for a setup step or for updating, shown and run in the terminal.
pub fn script(step: &str) -> Option<String> {
    Some(match step {
        "updates" => format!("{UPDATES}{UPDATE_LOG}"),
        "update-log" => UPDATE_LOG.to_string(),
        "libvirt" => LIBVIRT.to_string(),
        "power" => POWER.to_string(),
        "desktop" => DESKTOP.to_string(),
        _ => return None,
    })
}

/// Updating runs on the server as the transient service devbench-update, so it goes on when
/// this terminal closes or the connection drops; its output goes to /var/log/devbench-update.log,
/// which DevBench reads for the progress and the result. The terminal follows that log.
const UPDATES: &str = r#"if systemctl is-active --quiet devbench-update; then
  echo '== An update is already running, following it =='
else
  echo '== Updating packages (apt, then snaps) =='
  echo 'This runs on the server as the service devbench-update: closing this terminal does not stop it.'
  TMP="$(mktemp)"
  cat > "$TMP" <<'RUN'
#!/bin/bash
# Written by DevBench: installs all package and snap updates, output in /var/log/devbench-update.log
export LC_ALL=C DEBIAN_FRONTEND=noninteractive NEEDRESTART_MODE=l
exec >/var/log/devbench-update.log 2>&1
echo "@@start $(date +%s)"
code=0
step() { echo; echo "== $*"; "$@" || code=$?; }
step apt-get update
[ $code -eq 0 ] && step apt-get -y -o Dpkg::Options::=--force-confdef -o Dpkg::Options::=--force-confold full-upgrade
[ $code -eq 0 ] && step apt-get -y autoremove
[ $code -eq 0 ] && command -v snap >/dev/null 2>&1 && step snap refresh
[ -f /var/run/reboot-required ] && { echo; echo '!! A restart is needed to finish the updates.'; }
echo "@@done $code $(date +%s)"
exit $code
RUN
  sudo install -m 0755 -o root -g root "$TMP" /usr/local/sbin/devbench-update
  rm -f "$TMP"
  sudo systemd-run --unit=devbench-update --collect --quiet /usr/local/sbin/devbench-update
  sleep 1
fi
"#;

/// Shows the update log: follows it while the update runs, then says how it ended.
const UPDATE_LOG: &str = r#"L=/var/log/devbench-update.log
if [ ! -r "$L" ]; then echo 'DevBench has not run an update on this server yet.'; exit 0; fi
pid="$(systemctl show -p MainPID --value devbench-update 2>/dev/null)"
if [ -n "$pid" ] && [ "$pid" != 0 ]; then
  tail -n +1 -f --pid="$pid" "$L" | grep --line-buffered -v '^@@' || true
else
  grep -v '^@@' "$L" || true
fi
echo
r="$(sed -n 's/^@@done \([0-9]*\).*/\1/p' "$L" | tail -n1)"
grep -E '^[0-9]+ upgraded' "$L" | sed 's/^/   /'
[ -n "$r" ] || { echo 'The update did not finish (cut off, e.g. by a restart).'; exit 1; }
[ "$r" -eq 0 ] || { echo "The update stopped at an error (exit code $r)."; exit "$r"; }
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
        assert!(c.last_update.is_none() && !c.updating);
    }

    #[test]
    fn parses_last_update() {
        let out = "pkgs|libvirt0 qemu-system-x86 firefox\nlast|1700000000|0|1700000600|12|1|0\nrunning|1\n";
        let c = parse(out);
        assert_eq!(c.packages, ["libvirt0", "qemu-system-x86", "firefox"]);
        assert!(c.updating);
        let l = c.last_update.unwrap();
        assert_eq!((l.started, l.finished, l.code, l.upgraded, l.phased, l.kept_back), (Some(1700000000), Some(1700000600), Some(0), 12, true, false));
        // Cut off: started, never finished.
        let l = parse("last|1700000000||||0|0\n").last_update.unwrap();
        assert_eq!((l.started, l.finished, l.code), (Some(1700000000), None, None));
    }

    #[test]
    fn steps() {
        for s in ["updates", "update-log", "libvirt", "power", "desktop"] {
            let cmd = terminal_command(s).unwrap();
            assert!(cmd.starts_with("bash -lc '"));
        }
        assert!(terminal_command("rm -rf /").is_none());
    }
}
