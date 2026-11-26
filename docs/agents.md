# DevBench für Claude und andere KI-Agenten

*English summary at the end.*

DevBench lässt sich nicht nur per Maus bedienen, sondern auch von KI-Agenten wie Claude Code, Claude Desktop, Cursor oder Codex. Dafür bringt DevBench einen **MCP-Server** mit ([Model Context Protocol](https://modelcontextprotocol.io/)). Ein Agent kann dann z. B.:

- den Status der Server abfragen, einen Server aufwecken oder eine VM starten,
- Befehle auf dem Server oder **in der VM** ausführen (Linux-Shell, unter Windows PowerShell/cmd) und Dateien lesen und schreiben,
- **Screenshots** vom Bildschirm einer VM machen und sie mit Maus und Tastatur bedienen, um mit grafischen Programmen zu arbeiten.

Befehle und Dateien laufen direkt über SSH und sind deshalb viel schneller als der Umweg über den Bildschirm. Screenshots, Maus und Tastatur sind für Programme gedacht, die nur eine Oberfläche haben.

Agenten sehen **nichts**, bis man es pro Server und pro VM ausdrücklich freigibt (siehe [Freigaben](#freigaben)).

## Einrichten

Der MCP-Server ist dasselbe Programm wie die App, mit dem Argument `mcp`. Er läuft als eigener Prozess (stdio), liest dieselbe `config.toml` und baut eigene SSH-Verbindungen auf. Die App muss dafür nicht geöffnet sein.

### Claude Code

```sh
claude mcp add devbench -- "<Installationsordner>\devbench.exe" mcp
```

`<Installationsordner>` ist der Ordner der installierten App. Der Windows-Installer legt sie standardmäßig für den aktuellen Benutzer unter `%LOCALAPPDATA%` ab; im Zweifel im Startmenü per Rechtsklick → *Dateispeicherort öffnen* nachsehen. Linux (AppImage): `claude mcp add devbench -- /pfad/zu/DevBench.AppImage mcp`. macOS: `claude mcp add devbench -- /Applications/DevBench.app/Contents/MacOS/devbench mcp`.

### Claude Desktop, Cursor und andere

In die MCP-Konfiguration des jeweiligen Programms (bei Claude Desktop `claude_desktop_config.json`):

```json
{
  "mcpServers": {
    "devbench": {
      "command": "<Installationsordner>\\devbench.exe",
      "args": ["mcp"]
    }
  }
}
```

## Freigaben

Alle Freigaben sind standardmäßig **aus** und werden im Einstellungs-Bildschirm der App (oder in `config.toml`) gesetzt.

| Ebene | Feld | Erlaubt |
|-------|------|---------|
| Server | `agent_access` | Server ist für Agenten sichtbar: Status, Aufwecken, VMs auflisten |
| Server | `agent_exec` | Befehle ausführen und Dateien lesen/schreiben auf dem Server |
| Server | `agent_power` | Server herunterfahren/neu starten und VMs hart ausschalten (`destroy`) |
| VM | `agent_access` | VM starten, herunterfahren, neu starten; Screenshots, Maus und Tastatur |
| VM | `agent_exec` | Befehle ausführen und Dateien lesen/schreiben im Gast |

Eine VM ist nur erreichbar, wenn auch ihr Server `agent_access = true` hat.

```toml
[[machines]]
id = "server-1"
# ...
agent_access = true
agent_exec = true
agent_power = false

  [[machines.vms]]
  id = "vm-1"
  # ...
  agent_access = true
  agent_exec = true
```

`agent_exec` gibt einem Agenten eine Shell mit den Rechten des SSH-Benutzers. Nur dort einschalten, wo man das will, z. B. in einer Test-VM.

## Protokoll

Jede Aktion eines Agenten wird als eine JSON-Zeile in `activity.log` im Konfigurationsordner angehängt (Zeit, Werkzeug, Server, VM, Kurzbeschreibung, Erfolg oder Fehler). Die App zeigt die letzten Einträge an, so sieht man jederzeit, was ein Agent getan hat.

## Werkzeuge

`machine` und `vm` nehmen jeweils die `id` oder den `name` aus der Konfiguration.

| Werkzeug | Parameter | Wirkung | Freigabe |
|----------|-----------|---------|----------|
| `list_machines` | – | Server und VMs, die für Agenten freigegeben sind | Server `agent_access` |
| `machine_status` | `machine` | erreichbar, Laufzeit, CPU, RAM, Festplatte, VM-Zustände | Server `agent_access` |
| `wake_machine` | `machine` | Wake-on-LAN | Server `agent_access` |
| `power_machine` | `machine`, `action`: `poweroff` \| `reboot` | Server herunterfahren / neu starten | Server `agent_power` |
| `vm_action` | `machine`, `vm`, `action`: `start` \| `shutdown` \| `reboot` \| `destroy` | VM steuern (`destroy` = hart aus) | VM `agent_access`; `destroy` zusätzlich Server `agent_power` |
| `exec` | `machine`, `vm`?, `command`, `timeout_secs`? | Befehl ausführen; mit `vm` im Gast. Gibt stdout, stderr und Exit-Code zurück | `agent_exec` (Server bzw. VM) |
| `read_file` | `machine`, `vm`?, `path` | Datei lesen (SFTP) | `agent_exec` |
| `write_file` | `machine`, `vm`?, `path`, `content` | Datei schreiben (SFTP) | `agent_exec` |
| `vm_screenshot` | `machine`, `vm`, `max_size`? (Standard 1280) | PNG des VM-Bildschirms plus dessen echte Größe | VM `agent_access` |
| `vm_click` | `machine`, `vm`, `x`, `y`, `button`?: `left` \| `right` \| `middle`, `double`? | Mausklick | VM `agent_access` |
| `vm_move` | `machine`, `vm`, `x`, `y` | Maus bewegen | VM `agent_access` |
| `vm_scroll` | `machine`, `vm`, `x`, `y`, `dy` | scrollen | VM `agent_access` |
| `vm_type` | `machine`, `vm`, `text` | Text tippen | VM `agent_access` |
| `vm_key` | `machine`, `vm`, `keys` | Tastenkombination, z. B. `enter`, `win+r`, `ctrl+alt+delete` | VM `agent_access` |

Koordinaten gelten in den Pixeln des echten VM-Bildschirms, auch wenn der Screenshot verkleinert ist. Die echte Größe steht in der Antwort von `vm_screenshot`.

Bildschirm, Maus und Tastatur nutzen denselben VNC-Zugang wie die App (`vnc_mode` der VM). Befehle und Dateien im Gast brauchen SSH im Gast (`guest_ssh_user`), siehe [server-setup.md](server-setup.md).

## Tipps für die Arbeit mit Agenten

- Wo es geht, `exec` statt Klicken: Programme per Kommandozeile starten, Logs lesen, Dateien direkt schreiben. Screenshots nur für reine Oberflächen-Programme.
- Unter Windows startet `exec` die Standard-Shell des OpenSSH-Servers (meist `cmd`). Für PowerShell: `powershell -NoProfile -Command "…"`.
- Programme mit Fenster, die per `exec` gestartet werden, laufen in der SSH-Sitzung und **nicht** auf dem sichtbaren Desktop. Für sichtbare Programme besser über die Oberfläche starten (z. B. `vm_key` `win+r`, dann `vm_type`).

---

## English summary

DevBench ships an **MCP server** in the same binary: `devbench mcp` (stdio). It reads the same `config.toml`, opens its own SSH connections and does not need the GUI running.

- **Claude Code:** `claude mcp add devbench -- "<install dir>\devbench.exe" mcp`. Other clients: `"command": "<path to devbench>", "args": ["mcp"]`.
- **Tools:** `list_machines`, `machine_status`, `wake_machine`, `power_machine`, `vm_action`, `exec` (on the server or, with `vm`, inside the guest over SSH), `read_file`, `write_file` (SFTP), `vm_screenshot` (PNG, default max 1280 px, plus the real screen size), `vm_click`, `vm_move`, `vm_scroll`, `vm_type`, `vm_key`. Coordinates are in the VM screen's own pixels. `machine`/`vm` accept id or name.
- **Permissions, all off by default:** machine `agent_access`, `agent_exec`, `agent_power`; VM `agent_access`, `agent_exec`. A VM is only reachable if its machine has `agent_access`. `agent_exec` gives the agent a shell as the SSH user.
- **Audit:** every agent action is appended as a JSON line to `activity.log` in the config folder; the app shows recent entries.
- Prefer `exec` over clicking. GUI programs started via `exec` on Windows run in the SSH session, not on the visible desktop.
