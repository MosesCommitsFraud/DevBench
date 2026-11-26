# Releases und automatische Updates

*English summary at the end.*

Installierte DevBench-Kopien aktualisieren sich selbst über GitHub Releases. Beim Start fragt die App still die Datei `latest.json` des neuesten Releases ab. Gibt es eine neuere Version, erscheint an **Einstellungen** in der Seitenleiste ein Badge „Update“, und die Karte **Version** in den Einstellungen zeigt Version und Änderungen mit **Jetzt aktualisieren**. Ein Klick lädt das Update (mit Fortschritt), prüft seine Signatur, installiert es und startet DevBench neu. Dort gibt es auch **Nach Updates suchen**.

Dahinter steckt das [Tauri-Updater-Plugin](https://v2.tauri.app/plugin/updater/). Jedes Update ist mit einem privaten Schlüssel signiert; die App enthält den passenden öffentlichen Schlüssel (`plugins.updater.pubkey` in [`src-tauri/tauri.conf.json`](../src-tauri/tauri.conf.json)) und lehnt alles ab, was nicht damit signiert ist.

## Einmalig einrichten

1. **Signaturschlüssel** (schon erledigt, liegt unter `~/.tauri/devbench.key`, der öffentliche Teil steht in `tauri.conf.json`). Neu erzeugen nur, wenn er verloren ist:

   ```sh
   npm run tauri signer generate -- --ci -w ~/.tauri/devbench.key
   ```

   Den privaten Schlüssel `~/.tauri/devbench.key` **nie** committen und sicher aufbewahren (z. B. im Passwort-Manager). Geht er verloren, können installierte Kopien keine Updates mehr annehmen und müssen einmal von Hand neu installiert werden. Nach einem neuen Schlüssel den Inhalt von `devbench.key.pub` in `tauri.conf.json` eintragen.

2. **Secret im GitHub-Repository:** *Settings* → *Secrets and variables* → *Actions* → *New repository secret*, Name `TAURI_SIGNING_PRIVATE_KEY`, Wert: der Inhalt von `~/.tauri/devbench.key`. Oder mit der GitHub CLI:

   ```sh
   gh secret set TAURI_SIGNING_PRIVATE_KEY -R MosesCommitsFraud/DevBench < ~/.tauri/devbench.key
   ```

   Hat der Schlüssel ein Passwort, zusätzlich `TAURI_SIGNING_PRIVATE_KEY_PASSWORD`. Ein beim Kopieren aus PowerShell eingeschleppter BOM oder Zeilenumbruch wird im Workflow entfernt.

Ein lokales `npm run tauri build` signiert die Update-Dateien ebenfalls und braucht dafür den Schlüssel in der Umgebung: `$env:TAURI_SIGNING_PRIVATE_KEY = Get-Content ~/.tauri/devbench.key -Raw`.

## Eine neue Version veröffentlichen

1. GitHub → *Actions* → **Release** → **Run workflow**, Version ohne `v` eintragen (z. B. `0.2.0`).
2. Der Workflow ([`.github/workflows/release.yml`](../.github/workflows/release.yml)) setzt die Version in `package.json`, `tauri.conf.json` und `Cargo.toml`, committet sie, legt den Tag `v0.2.0` an, sammelt die Commits seit dem letzten Release als Änderungsliste und baut für Windows, Linux und macOS (Apple Silicon und Intel). Ergebnis ist ein **Entwurf** (Draft) mit allen Installern, den signierten Update-Dateien und `latest.json`. Das dauert etwa 15–25 Minuten.
3. Unter *Releases* den Entwurf prüfen und **Publish release** klicken. Erst dann sehen installierte Apps das Update: `releases/latest/download/…` zeigt nur auf veröffentlichte Releases.

Alternativ geht weiterhin: Version von Hand setzen (`node .github/scripts/set-version.mjs 0.2.0`), committen und einen Tag `v0.2.0` pushen.

## Was Nutzer bekommen

| System | Erstinstallation | Update |
|--------|------------------|--------|
| Windows | `DevBench_x.y.z_x64-setup.exe` (NSIS) oder `.msi` | automatisch in der App (stiller Installer) |
| macOS | `.dmg` | automatisch in der App |
| Linux | `.AppImage`, `.deb`, `.rpm` | automatisch nur beim **AppImage**; `.deb`/`.rpm` neu herunterladen |

### Code-Signierung des Betriebssystems

Die Update-Signatur oben ist etwas anderes als die Code-Signierung durch Microsoft bzw. Apple. Ohne Code-Signing-Zertifikat:

- **Windows** zeigt beim ersten Installieren eine SmartScreen-Warnung („Unbekannter Herausgeber“) → *Weitere Informationen* → *Trotzdem ausführen*. Updates laufen danach ohne Warnung.
- **macOS** blockiert die App zunächst → im Finder Rechtsklick → *Öffnen*, oder *Systemeinstellungen* → *Datenschutz & Sicherheit* → *Trotzdem öffnen*.

---

## English summary

Installed copies update themselves from GitHub Releases via the Tauri updater plugin. On start the app checks quietly; an available update shows a badge on Settings and a Version card with release notes, "Update now" (with progress) and "Check for updates". Updates are verified against the public key in `tauri.conf.json`.

**One-time setup:** the signing key lives in `~/.tauri/devbench.key` (never commit it; losing it means installed copies cannot update). Add its content as the repository secret `TAURI_SIGNING_PRIVATE_KEY` (`gh secret set TAURI_SIGNING_PRIVATE_KEY < ~/.tauri/devbench.key`).

**Each release:** Actions → Release → Run workflow with a version (e.g. `0.2.0`). The workflow bumps the version, commits, tags, collects the commit list as release notes and builds Windows, Linux and macOS into a draft release with `latest.json`. Publish the draft to roll it out. Pushing a `vX.Y.Z` tag by hand still works. Linux auto-update works for the AppImage only.
