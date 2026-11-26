# Server und VMs für DevBench vorbereiten

*English summary at the end.*

DevBench installiert nichts auf dem Server. Es meldet sich per SSH an, liest den Zustand und führt ein paar Befehle aus. Diese Seite beschreibt, was auf Server und VMs vorhanden sein muss.

## Server (Linux)

- **OpenSSH-Server** mit Schlüssel-Anmeldung. Vom PC aus muss `ssh <host>` ohne Passwortabfrage klappen (Schlüssel ohne Passphrase oder im ssh-agent).
- **Linux mit `/proc`.** Für das Dashboard liest DevBench `/proc/uptime`, `/proc/stat`, `/proc/meminfo` und `df -P /`. Root-Rechte sind dafür nicht nötig.
- **Herunterfahren / Neustart:** DevBench ruft `sudo -n systemctl poweroff` bzw. `sudo -n systemctl reboot` auf. Dafür braucht es eine sudoers-Regel, die nur diese beiden Befehle ohne Passwort erlaubt: siehe [`sudoers-devbench`](sudoers-devbench). Ohne die Regel zeigt DevBench beim Klick einen Fehler, alles andere funktioniert.
- **Wake-on-LAN:** siehe [wake-on-lan.md](wake-on-lan.md).

### Host-Schlüssel

Beim ersten Verbinden prüft DevBench den Host-Schlüssel. Es liest `~/.ssh/known_hosts` (schreibt aber nie hinein) und seine eigene Datei `known_hosts` im Konfigurationsordner. Ist der Schlüssel unbekannt, zeigt die App den Fingerabdruck und fragt nach. Hat er sich geändert, bricht DevBench ab. Den Fingerabdruck auf dem Server prüfen:

```sh
ssh-keygen -lf /etc/ssh/ssh_host_ed25519_key.pub
```

## libvirt / KVM

- Der SSH-Benutzer muss in der Gruppe **`libvirt`** sein, damit `virsh -c qemu:///system …` ohne sudo geht:
  ```sh
  sudo usermod -aG libvirt "$USER"    # danach neu anmelden
  virsh -c qemu:///system list --all  # Test
  ```
- Die Knöpfe entsprechen:
  - **Starten:** `virsh start <vm>`
  - **Herunterfahren:** `virsh shutdown <vm>`: schickt ein ACPI-Signal; der Gast muss darauf reagieren (Windows tut das normalerweise, Linux braucht `acpid` oder systemd-logind).
  - **Neustart:** `virsh reboot <vm>`
  - **Ausschalten (erzwingen):** `virsh destroy <vm>`: wie Stecker ziehen; nicht gespeicherte Daten im Gast gehen verloren.
- `libvirt_name` in der Konfiguration ist der Name aus `virsh list --all`.
- **Gast-Adresse:** Ist `guest_address` leer, fragt DevBench libvirt (`virsh domifaddr`). Das liefert ohne weitere Einstellungen nur bei VMs im libvirt-NAT-Netz (`default`, 192.168.122.x) eine Adresse. Hängt die VM an einer Bridge, die Adresse fest in `guest_address` eintragen (oder im Gast den QEMU Guest Agent installieren).

## VNC in die VM

DevBench zeigt den Bildschirm der VM direkt im Fenster (noVNC). Es braucht weder einen separaten Tunnel noch ein externes VNC-Programm: DevBench baut die SSH-Weiterleitung selbst auf. Es gibt zwei Arten (`vnc_mode`):

### `vnc_mode = "guest"`: VNC-Server im Gast

Nötig, wenn der Gast eine durchgereichte Grafikkarte nutzt (GPU-Passthrough): QEMUs eigene VNC-Anzeige zeigt dann nicht den echten Bildschirm. Der Weg ist: PC → SSH zum Server → SSH weiter in den Gast → `vnc_host:vnc_port` im Gast.

**Windows-Gast:**

1. OpenSSH-Server installieren und starten (PowerShell als Administrator):
   ```powershell
   Add-WindowsCapability -Online -Name OpenSSH.Server~~~~0.0.1.0
   Set-Service sshd -StartupType Automatic
   Start-Service sshd
   ```
2. Öffentlichen Schlüssel des PCs hinterlegen. **Achtung:** Ist der Benutzer Administrator, liest Windows-OpenSSH *nicht* `~\.ssh\authorized_keys`, sondern `C:\ProgramData\ssh\administrators_authorized_keys`. Diese Datei darf nur für Administratoren und SYSTEM lesbar sein:
   ```powershell
   icacls C:\ProgramData\ssh\administrators_authorized_keys /inheritance:r /grant "Administratoren:F" /grant "SYSTEM:F"
   ```
   (In englischem Windows heißt die Gruppe `Administrators`.)
3. Einen VNC-Server installieren, z. B. **TightVNC** (frei) als Dienst. In den Einstellungen:
   - Reiter **Access Control**: „Allow loopback connections“ und „Allow only loopback connections“ einschalten. Dann ist VNC von außen gar nicht erreichbar, nur durch den SSH-Tunnel.
   - Port 5900 (Standard).
   - Ein VNC-Passwort ist optional; ist eins gesetzt, fragt das DevBench-Fenster danach. Sollen KI-Agenten den Bildschirm nutzen, muss es zusätzlich als `vnc_password` bei der VM eingetragen werden (Klartext in `config.toml`).
4. Vom Server aus testen: `ssh <gast-benutzer>@<gast-adresse>` muss mit dem Schlüssel klappen.

Mit durchgereichter GPU und ohne angeschlossenen Monitor stellt Windows oft nur eine kleine Auflösung oder gar keinen Desktop bereit. Abhilfe: ein HDMI/DisplayPort-Dummy-Stecker an der Grafikkarte oder ein virtueller Bildschirmtreiber.

**Linux-Gast:** OpenSSH-Server (`sudo apt install openssh-server`), Schlüssel in `~/.ssh/authorized_keys`, und ein VNC-Server, der nur auf 127.0.0.1 lauscht (z. B. `x11vnc -localhost` oder TigerVNC mit `-localhost`). Für Linux-Desktops ist später RDP geplant (siehe Roadmap in der README).

In DevBench: `guest_ssh_user`, ggf. `guest_address`, `vnc_host = "127.0.0.1"`, `vnc_port = 5900`.

### `vnc_mode = "host"`: QEMUs eigene VNC-Anzeige

Für VMs mit virtueller Grafik (ohne GPU-Passthrough). Im Gast ist nichts nötig. In der VM-Definition (`virsh edit <vm>`) eine VNC-Grafik, die nur auf localhost des Servers lauscht:

```xml
<graphics type='vnc' port='-1' autoport='yes' listen='127.0.0.1'/>
```

Den Port zeigt `virsh vncdisplay <vm>` an (`:0` = 5900, `:1` = 5901 …). Bei `autoport` kann er sich je nach Startreihenfolge ändern; für einen festen Port `port='5901' autoport='no'` setzen. In DevBench: `vnc_mode = "host"`, `vnc_host = "127.0.0.1"`, `vnc_port` = dieser Port.

## Terminal und Desktop des Servers

**Terminal:** Der Knopf *Terminal* an einer Maschine öffnet eine Shell über dieselbe SSH-Verbindung, an einer laufenden VM (mit `guest_ssh_user`) eine Shell im Gast. Auf dem Server ist dafür nichts nötig. Kopieren und Einfügen mit Strg+Umschalt+C / Strg+Umschalt+V.

**Desktop:** Hat der Server eine grafische Oberfläche, zeigt DevBench sie über VNC an, genau wie bei VMs durch SSH weitergeleitet. Der VNC-Server soll nur auf `127.0.0.1` lauschen. In den Einstellungen unter *Desktop* den Port eintragen (`desktop_vnc_port`, 0 = kein Desktop-Knopf).

Für einen Server ohne Monitor ist ein eigener virtueller Desktop am einfachsten, z. B. Xfce mit TigerVNC:

```sh
sudo apt install xfce4 xfce4-goodies tigervnc-standalone-server
vncpasswd                                   # Passwort für die VNC-Anzeige
vncserver :1 -localhost yes -geometry 1920x1080 -xstartup /usr/bin/startxfce4
```

Das ergibt Port 5901 (`:1`). Dauerhaft als Dienst: `sudo cp /usr/lib/systemd/system/vncserver@.service /etc/systemd/system/` (falls vorhanden) bzw. eine eigene `systemd`-Unit für den Benutzer anlegen. Auf einem Rechner mit GNOME-Desktop geht stattdessen `gnome-remote-desktop` (VNC-Backend) oder `wayvnc` (Sway/wlroots), jeweils auf localhost.

---

## English summary

- **Server:** OpenSSH with key auth, Linux with `/proc`. Shut down/reboot need the sudoers drop-in in [`sudoers-devbench`](sudoers-devbench). Wake-on-LAN: [wake-on-lan.md](wake-on-lan.md). Unknown host keys are shown with their fingerprint and must be confirmed; changed keys are refused.
- **libvirt:** add the SSH user to group `libvirt`. Buttons map to `virsh start/shutdown/reboot/destroy`. Empty `guest_address` uses `virsh domifaddr`, which only works out of the box on libvirt's NAT network; set it explicitly for bridged VMs.
- **VNC, `guest` mode** (needed with GPU passthrough): OpenSSH server in the guest (Windows admins: key goes into `C:\ProgramData\ssh\administrators_authorized_keys`), a VNC server such as TightVNC listening on loopback only. A dummy display plug helps if no monitor is attached to the passed-through GPU.
- **VNC, `host` mode:** QEMU's own VNC display with `listen='127.0.0.1'`; port from `virsh vncdisplay`.
- **Terminal and desktop of the server:** *Terminal* opens a shell over SSH (on the machine, or in a running guest with `guest_ssh_user`); nothing to install. *Desktop* shows a VNC server running on the machine itself, forwarded through SSH: set `desktop_vnc_port` (0 hides the button). For a headless server, e.g. Xfce plus TigerVNC: `vncserver :1 -localhost yes -xstartup /usr/bin/startxfce4` gives port 5901.
