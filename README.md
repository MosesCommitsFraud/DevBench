<p align="center"><img src="docs/brand/logo.svg" width="120" alt=""></p>

# DevBench

Eine Desktop-App als Schaltzentrale für Heimserver und ihre virtuellen Maschinen: Status auf einen Blick, Server per Wake-on-LAN aufwecken, herunterfahren, VMs starten und stoppen und den Bildschirm einer VM direkt im Fenster bedienen. Alles läuft über SSH, ohne Hilfsprogramme auf dem Server.

> **Status:** frühe Entwicklung (0.1). Getestet unter Windows 11 mit einem Ubuntu-Server und libvirt/KVM.

## English summary

DevBench is a cross-platform desktop app (Tauri 2, Rust + Svelte) that acts as a control center for one or more home servers and their libvirt/KVM virtual machines.

- **Dashboard:** online/offline, uptime, CPU, RAM and disk per server; running state per VM.
- **Buttons:** Wake-on-LAN, shut down / reboot the server (over SSH, via a narrow sudoers rule), start / shut down / reboot / force off VMs (`virsh` over SSH).
- **Terminal:** a shell on the server or inside a guest, in the app window (xterm.js over the same SSH connection).
- **Embedded VNC:** view and control a VM's desktop, or the server's own desktop, inside the app (noVNC). DevBench opens the SSH tunnel itself, in-process (via [russh](https://github.com/Eugeny/russh)), including jump hosts. No external tunnel, no separate VNC viewer.
- **Files:** one file browser for this PC, every server and every VM guest (SFTP): pick a system from the list, breadcrumbs, filter, sort, upload/download, copy/cut/paste across systems, drag rows onto a system, drop files in from Explorer/Finder; transfers with progress and cancel.
- **Light and dark theme** from the logo colours, following the system or chosen in Settings.
- **AI agents (MCP):** `devbench mcp` is an MCP server, so Claude and other agents can check status, wake machines, start VMs, run commands and edit files on the server or inside a guest, and take screenshots and drive mouse and keyboard in a VM. Everything is off until enabled per machine/VM; the app shows connected agents and logs every agent action. See [`docs/agents.md`](docs/agents.md).
- **Nothing hardcoded:** all hosts live in a config file in your OS's app config folder, edited in the app's Settings screen; hosts can be imported from `~/.ssh/config`. See [`config.example.toml`](config.example.toml).

**Install:** installers for Windows, macOS and Linux are attached to GitHub Releases; installed copies check for updates on start, show a badge and update themselves from Settings (Linux: AppImage). Releases are started from GitHub Actions with a version number, see [`docs/releasing.md`](docs/releasing.md).

**Build:** install Rust (1.85+), Node.js (20+) and the [Tauri prerequisites](https://v2.tauri.app/start/prerequisites/) for your OS, then `npm install` and `npm run tauri dev` (development) or `npm run tauri build` (installer).

**Server setup:** OpenSSH with key auth; SSH user in group `libvirt`; for shut down/reboot install [`docs/sudoers-devbench`](docs/sudoers-devbench); for Wake-on-LAN see [`docs/wake-on-lan.md`](docs/wake-on-lan.md); for VNC into VMs see [`docs/server-setup.md`](docs/server-setup.md). The guides are in German with an English summary at the end of each.

---

## Funktionen

- **Dashboard** pro Server: erreichbar oder nicht, Betriebssystem, Prozessor, IP-Adresse, Laufzeit, CPU-Verlauf der letzten Minuten, Arbeitsspeicher, Festplatte und Netzwerk. Pro VM: Zustand, vCPUs, Arbeitsspeicher, IP-Adresse und CPU-Last.
- **Terminal:** Shell auf dem Server oder in einer VM direkt im Fenster, über dieselbe SSH-Verbindung.
- **Desktop des Servers:** hat der Server eine grafische Oberfläche mit VNC-Server, zeigt DevBench sie wie eine VM an, siehe [`docs/server-setup.md`](docs/server-setup.md#terminal-und-desktop-des-servers).
- **Server:** Aufwecken (Wake-on-LAN), Herunterfahren, Neustarten.
- **VMs (libvirt/KVM):** Starten, Herunterfahren (ACPI), Neustarten, Ausschalten erzwingen.
- **VNC im Fenster:** Bildschirm einer VM mit Maus und Tastatur bedienen. DevBench baut die SSH-Verbindung selbst auf, auch über einen Sprung-Server (Jump Host) in den Gast. Kein `ssh -L` im Terminal, kein separates VNC-Programm.
- **Dateien:** ein Dateibrowser für diesen PC, alle Server und alle VMs (SFTP), siehe [Dateien übertragen](#dateien-übertragen).
- **Hell und dunkel:** beide Designs aus den Logo-Farben; folgt dem System oder wird in den Einstellungen bzw. unten in der Seitenleiste gewählt.
- **KI-Agenten:** Claude und andere Agenten können Server und VMs über MCP bedienen, siehe [unten](#steuerung-durch-ki-agenten-mcp).
- **Mehrere Server**, jeder mit beliebig vielen VMs.
- **Import aus `~/.ssh/config`:** vorhandene Host-Aliasse übernehmen; Benutzer, Port, Schlüssel und ProxyJump werden von dort gelesen, wenn sie in DevBench leer bleiben.

## Installation

### Fertige Version

Installer für Windows, macOS und Linux liegen unter **Releases** im GitHub-Repository (sobald das erste Release veröffentlicht ist). Einmal installiert, sucht DevBench beim Start selbst nach neuen Versionen und installiert sie auf Knopfdruck. Unter Linux gilt das für das AppImage. Wie Releases entstehen: [`docs/releasing.md`](docs/releasing.md).

### Aus dem Quellcode bauen

#### Voraussetzungen

- [Rust](https://rustup.rs/) 1.85 oder neuer
- [Node.js](https://nodejs.org/) 20 oder neuer (mit npm)
- Die [Tauri-Voraussetzungen](https://v2.tauri.app/start/prerequisites/) für das eigene Betriebssystem:
  - **Windows:** Microsoft C++ Build Tools („Desktopentwicklung mit C++“) und WebView2 (bei Windows 10/11 meist schon installiert).
  - **Linux:** u. a. `libwebkit2gtk-4.1-dev`, `build-essential`, `libssl-dev`, `libayatana-appindicator3-dev`, `librsvg2-dev`.
  - **macOS:** Xcode Command Line Tools.

#### Bauen und starten

```sh
git clone https://github.com/MosesCommitsFraud/DevBench.git
cd DevBench
npm install
npm run tauri dev      # Entwicklungsmodus mit Hot Reload
npm run tauri build    # Installer bauen
```

Die Installer landen in `src-tauri/target/release/bundle/` (unter Windows `msi/` und `nsis/`).

## Konfiguration

DevBench speichert seine Einstellungen in einer TOML-Datei im Konfigurationsordner des Betriebssystems:

| System  | Pfad |
|---------|------|
| Windows | `%APPDATA%\DevBench\config.toml` |
| Linux   | `~/.config/DevBench/config.toml` |
| macOS   | `~/Library/Application Support/DevBench/config.toml` |

Beim ersten Start legt DevBench die Datei leer an. Server und VMs trägt man im Einstellungs-Bildschirm der App ein (oder importiert sie aus `~/.ssh/config`). Die Datei lässt sich auch von Hand bearbeiten; danach in der App „Neu laden“ drücken oder sie neu starten. Ein vollständiges, kommentiertes Beispiel steht in [`config.example.toml`](config.example.toml).

Die eigene Konfiguration gehört **nicht** ins Repository.

### Server (`[[machines]]`)

| Feld | Bedeutung |
|------|-----------|
| `id` | interne, eindeutige Kennung (vergibt die App) |
| `name` | Anzeigename |
| `ssh_host` | Hostname, IP oder ein `Host`-Alias aus `~/.ssh/config` |
| `ssh_port` | `0` = aus `~/.ssh/config`, sonst 22 |
| `ssh_user` | leer = aus `~/.ssh/config`, sonst der lokale Benutzername |
| `ssh_key` | leer = `IdentityFile` aus `~/.ssh/config`, sonst `~/.ssh/id_ed25519`, `id_ecdsa`, `id_rsa`; der ssh-agent wird auch versucht |
| `jump_host` | optional, wie `ProxyJump`: `[user@]host[:port]` oder Alias; leer = `ProxyJump` aus `~/.ssh/config` |
| `mac` | MAC-Adresse für Wake-on-LAN; leer = kein Aufwecken-Knopf |
| `wol_broadcast` | Broadcast-Adresse des Server-Netzes, z. B. `192.168.1.255` |
| `wol_port` | UDP-Port für das Magic Packet, meist `9` |
| `desktop_vnc_port`, `desktop_vnc_host` | VNC-Server mit dem Desktop des Servers selbst, durch SSH erreicht; `0` = kein Desktop-Knopf, Host leer = `127.0.0.1` |
| `agent_access`, `agent_exec`, `agent_power` | Freigaben für KI-Agenten, standardmäßig `false`, siehe [`docs/agents.md`](docs/agents.md) |

### VMs (`[[machines.vms]]`)

| Feld | Bedeutung |
|------|-----------|
| `id`, `name` | Kennung und Anzeigename |
| `libvirt_name` | Name der VM in `virsh list --all` |
| `libvirt_uri` | meist `qemu:///system` |
| `guest_address` | leer = libvirt fragen (`virsh domifaddr`), sonst IP/Hostname des Gasts, wie der Server ihn sieht |
| `guest_ssh_user`, `guest_ssh_port`, `guest_ssh_key` | SSH-Anmeldung im Gast (nur für `vnc_mode = "guest"`) |
| `vnc_mode` | `"guest"`: VNC-Server im Gast, Zugang per SSH durch den Server in den Gast. `"host"`: QEMUs eigene VNC-Anzeige auf dem Server. `"none"`: kein VNC |
| `vnc_host`, `vnc_port` | Ziel der Weiterleitung, vom Gast (`guest`) bzw. vom Server (`host`) aus gesehen; meist `127.0.0.1:5900` |
| `vnc_password` | nur nötig, wenn der VNC-Server ein Passwort verlangt und ein Agent den Bildschirm nutzen soll; steht im Klartext in der Datei (das Fenster der App fragt selbst nach) |
| `agent_access`, `agent_exec` | Freigaben für KI-Agenten in dieser VM, standardmäßig `false` |

### SSH-Host-Schlüssel

DevBench liest `~/.ssh/known_hosts` (ohne hineinzuschreiben) und eine eigene `known_hosts` im selben Ordner wie die Konfiguration. Unbekannte Schlüssel zeigt die App mit Fingerabdruck an und fragt, ob man ihnen vertraut; ein geänderter Schlüssel führt zu einem Fehler.

## Server einrichten

Kurzfassung, Details in [`docs/server-setup.md`](docs/server-setup.md):

1. **SSH:** OpenSSH-Server mit Schlüssel-Anmeldung; `ssh <host>` muss vom PC aus ohne Passwort klappen.
2. **libvirt:** SSH-Benutzer in die Gruppe `libvirt` aufnehmen (`sudo usermod -aG libvirt <user>`), damit `virsh` ohne sudo geht.
3. **Herunterfahren/Neustart:** sudoers-Regel aus [`docs/sudoers-devbench`](docs/sudoers-devbench) einrichten (siehe unten).
4. **Wake-on-LAN:** BIOS und Netzwerkkarte vorbereiten, siehe [`docs/wake-on-lan.md`](docs/wake-on-lan.md).
5. **VNC in VMs:** je nach Modus einen VNC-Server im Gast (z. B. TightVNC, nur auf localhost) oder QEMUs VNC-Anzeige, siehe [`docs/server-setup.md`](docs/server-setup.md#vnc-in-die-vm).

### sudoers-Regel für Herunterfahren und Neustart

DevBench führt `sudo -n systemctl poweroff` bzw. `sudo -n systemctl reboot` aus. Damit das ohne Passwort geht, aber **nichts anderes** erlaubt wird, auf dem Server eine eigene sudoers-Datei anlegen:

```sh
command -v systemctl                      # Pfad prüfen, meist /usr/bin/systemctl
sudo visudo -f /etc/sudoers.d/devbench
```

Inhalt (Benutzername anpassen):

```
<user> ALL=(root) NOPASSWD: /usr/bin/systemctl poweroff, /usr/bin/systemctl reboot
```

Danach `sudo chmod 0440 /etc/sudoers.d/devbench` und als dieser Benutzer mit `sudo -n -l` prüfen, dass genau diese zwei Befehle aufgelistet sind. `visudo` prüft die Syntax vor dem Speichern; eine kaputte sudoers-Datei kann sonst sudo komplett sperren. Die Vorlage mit Erklärung liegt in [`docs/sudoers-devbench`](docs/sudoers-devbench). Ohne diese Regel funktioniert alles außer diesen zwei Knöpfen.

### Wake-on-LAN

1. Im BIOS/UEFI *Wake on LAN* bzw. *Power On By PCI-E* einschalten und *ErP/EuP* ausschalten.
2. Unter Linux WoL auf der physischen Netzwerkkarte dauerhaft aktivieren (`ethtool -s <iface> wol g`, über NetworkManager, netplan oder systemd-networkd).
3. In DevBench `mac` und `wol_broadcast` eintragen. PC und Server müssen im selben lokalen Netz sein.

Schritt für Schritt: [`docs/wake-on-lan.md`](docs/wake-on-lan.md).

## Dateien übertragen

Unter **Dateien** in der Seitenleiste steht links die Liste der Systeme: **Dieser PC**, jeder Server und darunter seine VMs (nur VMs mit eingetragenem `guest_ssh_user`). Ein Klick wählt das System, rechts erscheint sein Dateibrowser. DevBench merkt sich pro System den zuletzt geöffneten Ordner. Die Übertragung läuft über SFTP durch dieselbe SSH-Verbindung, auf dem Server ist nichts zusätzlich nötig.

- **Navigieren:** über die Pfadleiste (Klick auf einen Teil springt dorthin, Klick daneben oder Strg+L zum Eintippen), über **Orte** (Home, Desktop, Downloads, Laufwerke, `/`), Zurück (Alt+←) und Nach oben (Alt+↑). In Windows-Gästen sehen Pfade so aus: `/C:/Users/name`.
- **Finden:** **Filtern** (Strg+F) blendet alles aus, was den Text nicht enthält; Klick auf eine Spaltenüberschrift sortiert.
- **Hoch- und herunterladen:** auf einem Server oder in einer VM lädt **Hochladen** Dateien von diesem PC in den offenen Ordner, **Herunterladen** speichert die Auswahl im Downloads-Ordner. Dateien aus dem Explorer/Finder lassen sich auch einfach auf die Liste oder auf ein System ziehen.
- **Zwischen Systemen:** **Kopieren** (Strg+C) oder **Ausschneiden** (Strg+X), anderes System wählen, Ordner öffnen, **Einfügen** (Strg+V). Oder Zeilen auf ein System in der Liste ziehen: sie werden in dessen zuletzt geöffneten Ordner kopiert (mit Umschalt verschoben). Innerhalb eines Systems verschiebt Ziehen auf einen Ordner (mit Umschalt kopiert).
- **Ordner:** neu, umbenennen (F2), löschen (Entf). Gelöschtes landet **nicht** im Papierkorb.
- **Übertragungen** erscheinen unten mit Fortschritt und lassen sich abbrechen.

## Steuerung durch KI-Agenten (MCP)

DevBench enthält einen MCP-Server, damit Claude und andere Agenten Server und VMs direkt bedienen können: Status, Aufwecken, VMs starten, Befehle und Dateien auf dem Server oder im Gast, und Screenshots, Maus und Tastatur für grafische Programme in der VM. Einrichten in Claude Code:

```sh
claude mcp add devbench -- "%LOCALAPPDATA%\DevBench\devbench.exe" mcp
```

Agenten sehen nur, was man pro Server und VM in den Einstellungen freigibt (`agent_access`, `agent_exec`, `agent_power`; alles standardmäßig aus). Unter **Verbundene Agenten** zeigt die App, welche Agenten gerade verbunden sind; jede Agenten-Aktion landet in `activity.log` und ist unter **Agenten-Aktivität** sichtbar. Alle Werkzeuge und Freigaben: [`docs/agents.md`](docs/agents.md).

## Sicherheit

- DevBench nutzt nur SSH mit Schlüsseln; Passwörter für SSH werden nicht gespeichert. Einzige Ausnahme ist das optionale `vnc_password` für Agenten, das im Klartext in `config.toml` steht.
- VNC-Server sollen nur auf `127.0.0.1` lauschen. DevBench leitet die Verbindung durch SSH weiter und öffnet lokal einen WebSocket nur auf `127.0.0.1` mit einem einmaligen Token.
- Die sudoers-Regel erlaubt genau zwei feste Befehle.
- KI-Agenten haben nur Zugriff, wo er ausdrücklich freigegeben ist; jede ihrer Aktionen wird protokolliert.
- DevBench ändert auf dem Server nichts von selbst: Es liest Status und führt nur die Befehle der Knöpfe aus, die man anklickt.

## Roadmap

Noch nicht eingebaut, geplant:

- **RDP** über [IronRDP](https://github.com/Devolutions/IronRDP) (Devolutions): in Windows-VMs und auf einen Linux-Desktop des Servers (gnome-remote-desktop im Headless-Modus oder xrdp), ebenfalls durch den SSH-Tunnel.
- **Logs** ansehen (journald des Servers, libvirt-Logs der VMs).

## Technik

- [Tauri 2](https://tauri.app/): Rust-Backend, Web-Frontend mit [Svelte 5](https://svelte.dev/) und TypeScript, gebaut mit Vite.
- [russh](https://github.com/Eugeny/russh): SSH-Client in Rust (Schlüssel, ssh-agent, Jump Hosts, Port-Weiterleitung im Prozess).
- [noVNC](https://github.com/novnc/noVNC): VNC-Client im Frontend; das Backend verbindet dessen WebSocket mit der SSH-Weiterleitung (wie websockify).

## Lizenz / License

DevBench steht wahlweise unter der [MIT-Lizenz](LICENSE-MIT) oder der [Apache-Lizenz 2.0](LICENSE-APACHE).
Beiträge werden, sofern nicht ausdrücklich anders angegeben, unter denselben beiden Lizenzen eingebracht.

DevBench is licensed under either of [MIT](LICENSE-MIT) or [Apache-2.0](LICENSE-APACHE) at your option.
Unless you explicitly state otherwise, any contribution you submit is dual-licensed as above, without additional terms.
