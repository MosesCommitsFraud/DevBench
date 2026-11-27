<p align="center"><img src="docs/brand/logo.svg" width="120" alt=""></p>

# DevBench

A desktop app that is the control center for your home servers and their virtual machines: status at a glance, wake a server over the network, shut it down, start and stop VMs, open a terminal or a VM's screen right in the window, and move files between your PC, the server and its guests. Everything runs over SSH, with nothing to install on the server.

> **Status:** early development (0.2). Tested on Windows 11 with an Ubuntu server and libvirt/KVM.

## Features

- **Dashboard** per server: online or not, operating system, IP address, uptime, CPU, memory, disk and network traffic. Per VM: state, vCPUs, memory, IP address, CPU load and a small live picture of its screen; for a running guest with SSH also what it reports itself (system, uptime, memory, system disk and, with an NVIDIA card, GPU load, VRAM and temperature), Windows or Linux.
- **Server:** wake (Wake-on-LAN), shut down, restart.
- **Setup & updates:** DevBench checks what it needs on a server (KVM/libvirt, the shutdown rule, the virtual desktop) and how many package updates wait; it sets up what is missing and installs updates in its terminal, where sudo asks for the password itself. Updates then run on the server as a service, so closing the terminal does not stop them, and the server's page shows their progress and the result. See [Setting up a server](#setting-up-a-server).
- **Port forwarding:** forward a port of a server or VM (a dev server on `:3000`, say) to `localhost` on this PC with one click, like `ssh -L`; DevBench lists what listens there. Forwards come back on the next start.
- **VMs (libvirt/KVM):** start, shut down, restart, force off. Shutting down and restarting run inside the guest over SSH when it is reachable (much faster than ACPI, especially for Windows), with ACPI as fallback.
- **Terminal:** a shell on the server or inside a guest, in the app window (xterm.js over the same SSH connection).
- **Screens over VNC:** view and control a VM's desktop, or the server's own desktop, in the window (noVNC). DevBench opens the SSH tunnel itself, in-process, including jump hosts: no `ssh -L` in a terminal, no separate VNC viewer.
- **Files:** one file browser for this PC, every server and every VM guest (SFTP), see [Transferring files](#transferring-files).
- **Command palette:** <kbd>Ctrl</kbd>+<kbd>K</kbd> opens terminals, screens and file browsers, starts VMs and wakes servers by keyboard.
- **AI agents (MCP):** Claude and other agents can operate servers and VMs, see [below](#ai-agents-mcp).
- **Light and dark theme**, following the system or chosen in Settings; a colour and monogram per server and VM.
- **Several servers**, each with any number of VMs.
- **Import from `~/.ssh/config`:** take over existing host aliases; user, port, key and ProxyJump are read from there when left empty in DevBench.
- **Updates:** installed copies check GitHub Releases on start and update themselves from Settings.

## Installation

### Ready-made builds

Installers for Windows, macOS and Linux are attached to the [GitHub Releases](https://github.com/MosesCommitsFraud/DevBench/releases). Once installed, DevBench looks for new versions on start; when one is available, a badge appears on **Settings**, and the **Version** card there installs it and restarts the app. On Linux this works for the AppImage. How releases are made: [`docs/releasing.md`](docs/releasing.md).

### Building from source

#### Requirements

- [Rust](https://rustup.rs/) 1.85 or newer
- [Node.js](https://nodejs.org/) 20 or newer (with npm)
- The [Tauri prerequisites](https://v2.tauri.app/start/prerequisites/) for your operating system:
  - **Windows:** Microsoft C++ Build Tools ("Desktop development with C++") and WebView2 (usually present on Windows 10/11).
  - **Linux:** among others `libwebkit2gtk-4.1-dev`, `build-essential`, `libssl-dev`, `libayatana-appindicator3-dev`, `librsvg2-dev`.
  - **macOS:** Xcode Command Line Tools.

#### Build and run

```sh
git clone https://github.com/MosesCommitsFraud/DevBench.git
cd DevBench
npm install
npm run tauri dev      # development mode with hot reload
npm run tauri build    # build the installers (signs the update bundles, see docs/releasing.md)
```

The installers end up in `src-tauri/target/release/bundle/` (on Windows in `msi/` and `nsis/`).

## Configuration

DevBench keeps its settings in a TOML file in your operating system's config folder:

| System  | Path |
|---------|------|
| Windows | `%APPDATA%\DevBench\config.toml` |
| Linux   | `~/.config/DevBench/config.toml` |
| macOS   | `~/Library/Application Support/DevBench/config.toml` |

On first start DevBench creates the file empty. You add servers and VMs in the app's Settings screen (or import them from `~/.ssh/config`). You can also edit the file by hand; then press "Reload from file" in the app or restart it. A complete, commented example is in [`config.example.toml`](config.example.toml).

Your own config does **not** belong in the repository. Nothing about any particular setup (hosts, MAC addresses, users) is hard-coded in DevBench.

### Servers (`[[machines]]`)

| Field | Meaning |
|-------|---------|
| `id` | internal unique id (set by the app) |
| `name` | display name |
| `color` | colour of its monogram, e.g. `"teal"`; empty = picked from the name |
| `ssh_host` | hostname, IP or a `Host` alias from `~/.ssh/config` |
| `ssh_port` | `0` = from `~/.ssh/config`, else 22 |
| `ssh_user` | empty = from `~/.ssh/config`, else your local user name |
| `ssh_key` | empty = `IdentityFile` from `~/.ssh/config`, else `~/.ssh/id_ed25519`, `id_ecdsa`, `id_rsa`; ssh-agent is tried too |
| `jump_host` | optional, like `ProxyJump`: `[user@]host[:port]` or an alias; empty = `ProxyJump` from `~/.ssh/config` |
| `mac` | MAC address for Wake-on-LAN; empty = no Wake button |
| `wol_broadcast` | broadcast address of the server's network, e.g. `192.168.1.255` |
| `wol_port` | UDP port for the magic packet, usually `9` |
| `desktop_vnc_port`, `desktop_vnc_host` | a VNC server showing the server's own desktop, reached through SSH; `0` = no Desktop button, empty host = `127.0.0.1` |
| `agent_access`, `agent_exec`, `agent_power` | permissions for AI agents, all `false` by default, see [`docs/agents.md`](docs/agents.md) |

### VMs (`[[machines.vms]]`)

| Field | Meaning |
|-------|---------|
| `id`, `name`, `color` | id, display name and monogram colour |
| `libvirt_name` | the VM's name in `virsh list --all` |
| `libvirt_uri` | usually `qemu:///system` |
| `guest_address` | empty = ask libvirt (`virsh domifaddr`), else the guest's IP/hostname as the server sees it |
| `guest_ssh_user`, `guest_ssh_port`, `guest_ssh_key` | SSH login into the guest (for the terminal, files, fast shutdown and `vnc_mode = "guest"`) |
| `vnc_mode` | `"guest"`: VNC server inside the guest, reached over SSH through the server into the guest. `"host"`: QEMU's own VNC display on the server. `"none"`: no VNC |
| `vnc_host`, `vnc_port` | where the forwarding goes, as seen from the guest (`guest`) or from the server (`host`); usually `127.0.0.1:5900` |
| `vnc_password` | only needed if the VNC server asks for one and an agent should use the screen; stored in plain text (the app window asks by itself) |
| `agent_access`, `agent_exec` | permissions for AI agents inside this VM, `false` by default |

### SSH host keys

DevBench reads `~/.ssh/known_hosts` (without writing to it) and its own `known_hosts` next to the config file. Unknown keys are shown with their fingerprint and you decide whether to trust them; a changed key is an error.

## Setting up a server

On an apt-based server (Debian, Ubuntu), most of this is one click: the server's menu (⋯) → **Setup & updates** shows what is there and runs the missing steps in the terminal (each step's commands can be shown first). The same dialog installs package and snap updates; a badge on the server shows when updates wait.

Short version of the manual way; the details are in [`docs/server-setup.md`](docs/server-setup.md) (the guides in `docs/` are in German with an English summary at the end of each).

1. **SSH:** OpenSSH server with key login; `ssh <host>` must work from your PC without a password.
2. **libvirt:** add the SSH user to the `libvirt` group (`sudo usermod -aG libvirt <user>`) so `virsh` works without sudo.
3. **Shut down/restart:** install the sudoers rule from [`docs/sudoers-devbench`](docs/sudoers-devbench) (see below).
4. **Wake-on-LAN:** prepare the BIOS and network card, see [`docs/wake-on-lan.md`](docs/wake-on-lan.md).
5. **VNC into VMs:** depending on the mode, a VNC server inside the guest (e.g. TightVNC, localhost only) or QEMU's VNC display, see [`docs/server-setup.md`](docs/server-setup.md#vnc-in-die-vm).
6. **The server's own desktop (optional):** a VNC server on the server, e.g. a virtual Xfce desktop with TigerVNC as a user service, see [`docs/server-setup.md`](docs/server-setup.md#terminal-und-desktop-des-servers).

### sudoers rule for shutting down and restarting

DevBench runs `sudo -n systemctl poweroff` and `sudo -n systemctl reboot`. To allow that without a password, but **nothing else**, create a sudoers file of its own on the server:

```sh
command -v systemctl                      # check the path, usually /usr/bin/systemctl
sudo visudo -f /etc/sudoers.d/devbench
```

Content (adjust the user name):

```
<user> ALL=(root) NOPASSWD: /usr/bin/systemctl poweroff, /usr/bin/systemctl reboot
```

Then `sudo chmod 0440 /etc/sudoers.d/devbench`, and as that user check with `sudo -n -l` that exactly these two commands are listed. `visudo` checks the syntax before saving; a broken sudoers file can lock sudo out completely. The template with explanations is in [`docs/sudoers-devbench`](docs/sudoers-devbench). Without this rule everything except these two buttons works.

### Wake-on-LAN

1. In the BIOS/UEFI, enable *Wake on LAN* or *Power On By PCI-E* and disable *ErP/EuP*.
2. On Linux, enable WoL on the physical network card permanently (`ethtool -s <iface> wol g`, via NetworkManager, netplan or systemd-networkd).
3. In DevBench, enter `mac` and `wol_broadcast`. PC and server must be on the same local network.

Step by step: [`docs/wake-on-lan.md`](docs/wake-on-lan.md).

## Transferring files

Under **Files** in the sidebar, the list on the left shows the systems: **This PC**, every server and, below it, its VMs (only VMs with a `guest_ssh_user`). A click picks the system, and its file browser opens on the right. DevBench remembers the last folder per system. Transfers run over SFTP through the same SSH connection; nothing extra is needed on the server. The **Files** buttons on the dashboard open a server or VM directly.

- **Navigating:** with the path bar (click a part to jump there; click next to it or press <kbd>Ctrl</kbd>+<kbd>L</kbd> to type a path), with **Places** (home, desktop, downloads) and **Drives** (every disk of the system with its label and free space: drive letters on Windows, every mounted disk such as `/data` on Linux), Back (<kbd>Alt</kbd>+<kbd>←</kbd>) and Up (<kbd>Alt</kbd>+<kbd>↑</kbd>). In Windows guests, paths look like `/C:/Users/name`.
- **Finding:** **Filter** (<kbd>Ctrl</kbd>+<kbd>F</kbd>) hides everything that does not contain the text; clicking a column header sorts.
- **Upload and download:** on a server or VM, **Upload** copies files from this PC into the open folder, **Download** saves the selection to your Downloads folder. You can also drag files from Explorer/Finder onto the list or onto a system.
- **Between systems:** **Copy** (<kbd>Ctrl</kbd>+<kbd>C</kbd>) or **Cut** (<kbd>Ctrl</kbd>+<kbd>X</kbd>), pick another system, open a folder, **Paste** (<kbd>Ctrl</kbd>+<kbd>V</kbd>). Or drag rows onto a system in the list: they are copied into its last open folder (moved with <kbd>Shift</kbd>). Within one system, dragging onto a folder moves (copies with <kbd>Shift</kbd>).
- **Folders:** new, rename (<kbd>F2</kbd>), delete (<kbd>Del</kbd>). Deleted files do **not** go to a recycle bin.
- **Transfers** appear at the bottom with progress and can be cancelled.

## Port forwarding

**Ports** (at a server, or the plug icon at a running VM) lists what is forwarded and what listens on that system right now (`ss` on Linux, `Get-NetTCPConnection` on Windows), each with **Forward**; any other port can be typed in. The port appears on `localhost` of this PC, with the same number when it is free. A forwarded port shows as a small chip at the server or VM; a click opens it in the browser.

The connection runs through the existing SSH connection: to a guest with a `guest_ssh_user` through the guest's own SSH (so dev servers that listen only on the guest's localhost work), otherwise from the server to the guest's address. The listener on this PC costs nothing until something connects, so forwards stay set while a server sleeps and are opened again when DevBench starts.

## AI agents (MCP)

DevBench contains an MCP server so Claude and other agents can operate servers and VMs directly: status, waking, starting VMs, commands and files on the server or in a guest, and screenshots, mouse and keyboard for graphical programs in a VM. To set it up in Claude Code:

```sh
claude mcp add devbench -- "%LOCALAPPDATA%\DevBench\devbench.exe" mcp
```

Agents only see what you allow per server and VM in Settings (`agent_access`, `agent_exec`, `agent_power`; all off by default). The app lists the agents connected right now, and every agent action is written to `activity.log` and shown under **Agent activity**. All tools and permissions: [`docs/agents.md`](docs/agents.md).

## Security

- DevBench uses SSH with keys only; SSH passwords are never stored. The one exception is the optional `vnc_password` for agents, stored in plain text in `config.toml`.
- VNC servers should listen on `127.0.0.1` only. DevBench forwards the connection through SSH and opens a local WebSocket on `127.0.0.1` only, with a one-time token.
- The sudoers rule allows exactly two fixed commands.
- AI agents only get access where it is explicitly allowed, and every action they take is logged.
- Updates are verified against the public key built into the app before they are installed.
- DevBench changes nothing on the server by itself: it reads the status and only runs the commands behind the buttons you click.

## Roadmap

Not built yet, planned:

- **RDP** via [IronRDP](https://github.com/Devolutions/IronRDP) (Devolutions): into Windows VMs and onto a Linux desktop on the server, also through the SSH tunnel.
- **Logs:** journald of the server, libvirt logs of the VMs.

## Technology

- [Tauri 2](https://tauri.app/): Rust backend, web frontend with [Svelte 5](https://svelte.dev/) and TypeScript, built with Vite.
- [russh](https://github.com/Eugeny/russh): SSH client in Rust (keys, ssh-agent, jump hosts, in-process port forwarding, SFTP).
- [noVNC](https://github.com/novnc/noVNC): VNC client in the frontend; the backend bridges its WebSocket to the SSH forwarding (like websockify).
- [xterm.js](https://xtermjs.org/): the terminal.

## License

DevBench is © 2026 Moritz Schäfer and licensed under either of [MIT](LICENSE-MIT) or [Apache-2.0](LICENSE-APACHE) at your option.

Unless you explicitly state otherwise, any contribution you submit is dual-licensed as above, without additional terms.
