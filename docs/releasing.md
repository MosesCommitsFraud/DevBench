# Releases und automatische Updates

*English summary at the end.*

Installierte DevBench-Kopien aktualisieren sich selbst über GitHub Releases. Beim Start fragt die App die Datei `latest.json` des neuesten Releases ab. Gibt es eine neuere Version, zeigt sie „Update verfügbar“. Ein Klick lädt das Update, prüft seine Signatur, installiert es und startet DevBench neu.

Dahinter steckt das [Tauri-Updater-Plugin](https://v2.tauri.app/plugin/updater/). Jedes Update ist mit einem privaten Schlüssel signiert. Die App enthält den passenden öffentlichen Schlüssel und lehnt alles ab, was nicht damit signiert ist.

## Einmalig einrichten

### 1. Signaturschlüssel erzeugen

Auf dem eigenen PC, **nicht** im Repository-Ordner speichern:

```sh
npm run tauri signer generate -- -w ~/.tauri/devbench.key
```

Das fragt nach einem Passwort und legt zwei Dateien an:

- `~/.tauri/devbench.key`: privater Schlüssel. **Geheim**, nie committen, sicher sichern (z. B. im Passwort-Manager).
- `~/.tauri/devbench.key.pub`: öffentlicher Schlüssel. Kommt in die App.

> **Wichtig:** Geht der private Schlüssel verloren, können bereits installierte Kopien keine Updates mehr annehmen. Die Nutzer müssten DevBench dann einmal von Hand neu installieren.

### 2. Öffentlichen Schlüssel und Adresse in die App eintragen

Die Update-Einstellungen stehen in einer eigenen Datei, [`src-tauri/tauri.release.conf.json`](../src-tauri/tauri.release.conf.json). Nur der Release-Workflow nutzt sie (`--config src-tauri/tauri.release.conf.json`). Wer DevBench selbst aus dem Quellcode baut, braucht keinen Schlüssel und bekommt eine App ohne Update-Funktion.

Dort zwei Platzhalter ersetzen:

```json
"plugins": {
  "updater": {
    "pubkey": "<Inhalt von devbench.key.pub, die ganze Zeile>",
    "endpoints": [
      "https://github.com/OWNER/REPO/releases/latest/download/latest.json"
    ]
  }
}
```

- `REPLACE_WITH_THE_CONTENT_OF_devbench.key.pub` durch den **Inhalt** der `.pub`-Datei ersetzen, keinen Dateipfad.
- `OWNER/REPO` durch das eigene GitHub-Repository ersetzen, z. B. `maxmuster/DevBench`.
- `bundle.createUpdaterArtifacts: true` steht schon in der Datei.

Der öffentliche Schlüssel darf ins Repository, er ist nicht geheim.

### 3. Secrets im GitHub-Repository anlegen

GitHub → Repository → *Settings* → *Secrets and variables* → *Actions* → *New repository secret*:

| Name | Wert |
|------|------|
| `TAURI_SIGNING_PRIVATE_KEY` | Inhalt von `~/.tauri/devbench.key` |
| `TAURI_SIGNING_PRIVATE_KEY_PASSWORD` | das Passwort aus Schritt 1 |

`GITHUB_TOKEN` stellt GitHub selbst bereit. Unter *Settings* → *Actions* → *General* → *Workflow permissions* muss „Read and write permissions“ erlaubt sein (oder der Workflow setzt es selbst, wie hier mit `permissions: contents: write`).

## Eine neue Version veröffentlichen

1. Versionsnummer an **beiden** Stellen erhöhen, gleich lautend:
   - `src-tauri/tauri.conf.json` → `version`
   - `package.json` → `version`
   (`src-tauri/Cargo.toml` → `version` am besten mitziehen.)
2. Committen und einen passenden Tag pushen:
   ```sh
   git commit -am "Release 0.2.0"
   git tag v0.2.0
   git push origin main --tags
   ```
3. Der Workflow [`.github/workflows/release.yml`](../.github/workflows/release.yml) baut für Windows, Linux und macOS (Apple Silicon und Intel) und legt einen **Entwurf** (Draft) eines Releases mit allen Dateien und `latest.json` an. Das dauert etwa 15–25 Minuten.
4. Unter *Releases* den Entwurf prüfen, Änderungen in die Beschreibung schreiben und **Publish release** klicken. Erst dann sehen installierte Apps das Update: Die Adresse `releases/latest/download/…` zeigt nur auf veröffentlichte Releases.

Die Version im Tag (`v0.2.0`) und in `tauri.conf.json` (`0.2.0`) müssen übereinstimmen. Der Updater vergleicht die Version in `latest.json` mit der installierten Version.

## Was Nutzer bekommen

| System | Erstinstallation | Update |
|--------|------------------|--------|
| Windows | `DevBench_x.y.z_x64-setup.exe` (NSIS) oder `.msi` | automatisch in der App |
| macOS | `.dmg` | automatisch in der App |
| Linux | `.AppImage`, `.deb`, `.rpm` | automatisch nur beim **AppImage**; `.deb`/`.rpm` neu herunterladen |

### Code-Signierung des Betriebssystems

Die Update-Signatur oben ist etwas anderes als die Code-Signierung durch Microsoft bzw. Apple. Ohne Code-Signing-Zertifikat:

- **Windows** zeigt beim ersten Installieren eine SmartScreen-Warnung („Unbekannter Herausgeber“) → *Weitere Informationen* → *Trotzdem ausführen*. Updates laufen danach ohne Warnung.
- **macOS** blockiert die App zunächst → im Finder Rechtsklick → *Öffnen*, oder *Systemeinstellungen* → *Datenschutz & Sicherheit* → *Trotzdem öffnen*.

Code-Signing lässt sich später ergänzen (siehe [Tauri: Windows-](https://v2.tauri.app/distribute/sign/windows/) und [macOS-Signierung](https://v2.tauri.app/distribute/sign/macos/)); der Workflow braucht dann zusätzliche Secrets.

---

## English summary

Installed copies update themselves from GitHub Releases via the Tauri updater plugin; updates are verified against a public key compiled into the app.

**One-time setup:**
1. `npm run tauri signer generate -- -w ~/.tauri/devbench.key` (outside the repo). Keep the private key safe: losing it means installed copies can no longer update.
2. In `src-tauri/tauri.release.conf.json` (used only by the release workflow via `--config`): set `pubkey` to the *contents* of `devbench.key.pub` and replace `OWNER/REPO` in the endpoint. Builds from source without that file have no updater and need no key.
3. Add repository secrets `TAURI_SIGNING_PRIVATE_KEY` (key file contents) and `TAURI_SIGNING_PRIVATE_KEY_PASSWORD`.

**Each release:** bump `version` in `tauri.conf.json` and `package.json` (and `Cargo.toml`), commit, push a tag `vX.Y.Z`. The workflow builds Windows, Linux and macOS and creates a draft release with `latest.json`. Publish the draft; only then do installed apps see the update. Linux auto-update works for the AppImage only. Without OS code signing, Windows SmartScreen and macOS Gatekeeper warn on first install.
