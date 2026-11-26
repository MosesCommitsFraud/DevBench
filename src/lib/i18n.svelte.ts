// German and English interface text. The language follows the system unless chosen in Settings.

type Lang = "de" | "en";

const en = {
  "nav.machines": "Machines",
  "nav.activity": "Agent activity",
  "nav.settings": "Settings",

  "machine.online": "Online",
  "machine.offline": "Offline",
  "machine.checking": "Checking",
  "machine.up": "Up {0}",
  "machine.cpu": "CPU",
  "machine.memory": "Memory",
  "machine.disk": "Disk",
  "machine.load": "Load {0}",
  "machine.cores": "{0} cores",
  "machine.wake": "Wake",
  "machine.poweroff": "Shut down",
  "machine.reboot": "Restart",
  "machine.woke": "Wake-on-LAN packet sent to {0}",
  "machine.poweroff.confirm": "Shut down {0}",
  "machine.poweroff.body": "All running VMs on this machine stop with it. To start it again you need Wake-on-LAN or the power button.",
  "machine.reboot.confirm": "Restart {0}",
  "machine.reboot.body": "Running VMs stop and start again only if they are set to autostart.",
  "machine.unreachable": "Not reachable",
  "machine.novms": "No VMs configured",
  "machine.trust": "Check host key",

  "vm.running": "Running",
  "vm.screen": "Screen",
  "vm.start": "Start",
  "vm.shutdown": "Shut down",
  "vm.reboot": "Restart",
  "vm.destroy": "Force off",
  "vm.destroy.confirm": "Force {0} off",
  "vm.destroy.body": "Like pulling the power plug. Unsaved work inside the VM is lost.",
  "vm.shutdown.confirm": "Shut down {0}",
  "vm.shutdown.body": "The guest system is asked to shut down, as if the power button was pressed.",
  "vm.state.running": "Running",
  "vm.state.shut off": "Off",
  "vm.state.paused": "Paused",
  "vm.state.in shutdown": "Shutting down",
  "vm.state.crashed": "Crashed",
  "vm.state.pmsuspended": "Suspended",
  "vm.state.idle": "Idle",
  "vm.state.unknown": "Unknown",
  "vm.more": "More actions",

  "empty.title": "No machines yet",
  "empty.body": "Add a server in Settings, or import one from your SSH config.",
  "empty.action": "Open settings",

  "vnc.connecting": "Connecting to {0}",
  "vnc.disconnected": "Disconnected",
  "vnc.reconnect": "Reconnect",
  "vnc.close": "Close",
  "vnc.cad": "Ctrl+Alt+Del",
  "vnc.fit": "Fit to window",
  "vnc.actual": "Actual size",
  "vnc.fullscreen": "Full screen",
  "vnc.paste": "Paste clipboard as typing",
  "vnc.password": "VNC password",
  "vnc.password.body": "The VNC server asks for a password.",
  "vnc.connect": "Connect",
  "vnc.security": "VNC security failure: {0}",

  "hostkey.title": "Unknown server",
  "hostkey.body": "DevBench has not connected to {0} before. Compare the fingerprint with the server's key before you trust it, for example with ssh-keygen -lf /etc/ssh/ssh_host_ed25519_key.pub on the server.",
  "hostkey.trust": "Trust and connect",
  "hostkey.changed.title": "Host key changed",

  "confirm.cancel": "Cancel",

  "settings.title": "Settings",
  "settings.file": "Config file",
  "settings.openFolder": "Show in folder",
  "settings.reload": "Reload from file",
  "settings.save": "Save",
  "settings.saved": "Settings saved",
  "settings.discard": "Discard changes",
  "settings.unsaved": "Unsaved changes",
  "settings.language": "Language",
  "settings.language.auto": "System",
  "settings.addMachine": "Add machine",
  "settings.import": "Import from SSH config",
  "settings.import.none": "No hosts found in ~/.ssh/config",
  "settings.import.added": "Added",
  "settings.removeMachine": "Remove machine",
  "settings.removeVm": "Remove VM",
  "settings.addVm": "Add VM",
  "settings.detectVms": "Find VMs on this machine",
  "settings.detect.none": "No new VMs found",
  "settings.detect.save": "Save the machine first",

  "field.name": "Name",
  "field.sshHost": "SSH host",
  "field.sshHost.hint": "Hostname, IP address or an alias from ~/.ssh/config",
  "field.sshPort": "Port",
  "field.sshUser": "User",
  "field.sshKey": "Private key",
  "field.jump": "Jump host",
  "field.jump.hint": "user@host:port, or an alias. Empty uses ProxyJump from ~/.ssh/config",
  "field.fromSshConfig": "From SSH config",
  "field.mac": "MAC address",
  "field.mac.hint": "For Wake-on-LAN. Empty hides the Wake button",
  "field.broadcast": "Broadcast address",
  "field.wolPort": "WoL port",
  "field.libvirtName": "libvirt name",
  "field.libvirtUri": "libvirt URI",
  "field.guestAddress": "Guest address",
  "field.guestAddress.hint": "Empty asks libvirt for the guest's IP",
  "field.guestUser": "Guest SSH user",
  "field.guestPort": "Guest SSH port",
  "field.guestKey": "Guest private key",
  "field.sameAsMachine": "Same as machine",
  "field.vncMode": "VNC",
  "field.vncMode.guest": "Through the guest (SSH into the VM)",
  "field.vncMode.host": "From the machine (e.g. QEMU display)",
  "field.vncMode.none": "Off",
  "field.vncTarget": "VNC server",
  "field.vncPort": "VNC port",
  "section.connection": "Connection",
  "section.wol": "Wake-on-LAN",
  "section.vms": "Virtual machines",
  "section.agents": "AI agents",
  "agents.hint": "What Claude and other agents may do through devbench mcp. Everything is off by default.",
  "agent.access": "Can see and wake this machine",
  "agent.exec": "Can run commands and edit files",
  "agent.power": "Can shut down, restart and force VMs off",
  "agent.vm.access": "Can start and stop the VM, see its screen and use mouse and keyboard",
  "agent.vm.exec": "Can run commands and edit files in the guest",

  "activity.title": "Agent activity",
  "activity.empty": "No agent has used DevBench yet",
  "activity.hint": "Every action an AI agent takes through devbench mcp is listed here.",

  "update.available": "Version {0} is available",
  "update.install": "Install and restart",
  "update.installing": "Installing",

  "error.config": "The config file could not be read",
} as const;

type Key = keyof typeof en;

const de: Record<Key, string> = {
  "nav.machines": "Maschinen",
  "nav.activity": "Agenten-Aktivität",
  "nav.settings": "Einstellungen",

  "machine.online": "Online",
  "machine.offline": "Offline",
  "machine.checking": "Prüfe",
  "machine.up": "Läuft seit {0}",
  "machine.cpu": "CPU",
  "machine.memory": "Arbeitsspeicher",
  "machine.disk": "Festplatte",
  "machine.load": "Last {0}",
  "machine.cores": "{0} Kerne",
  "machine.wake": "Aufwecken",
  "machine.poweroff": "Herunterfahren",
  "machine.reboot": "Neu starten",
  "machine.woke": "Wake-on-LAN-Paket an {0} gesendet",
  "machine.poweroff.confirm": "{0} herunterfahren",
  "machine.poweroff.body": "Alle laufenden VMs auf dieser Maschine gehen mit aus. Zum Einschalten brauchst du Wake-on-LAN oder den Einschaltknopf.",
  "machine.reboot.confirm": "{0} neu starten",
  "machine.reboot.body": "Laufende VMs werden beendet und starten nur wieder, wenn Autostart eingestellt ist.",
  "machine.unreachable": "Nicht erreichbar",
  "machine.novms": "Keine VMs eingerichtet",
  "machine.trust": "Host-Schlüssel prüfen",

  "vm.running": "Läuft",
  "vm.screen": "Bildschirm",
  "vm.start": "Starten",
  "vm.shutdown": "Herunterfahren",
  "vm.reboot": "Neu starten",
  "vm.destroy": "Ausschalten erzwingen",
  "vm.destroy.confirm": "{0} hart ausschalten",
  "vm.destroy.body": "Wie Stecker ziehen. Nicht gespeicherte Arbeit in der VM geht verloren.",
  "vm.shutdown.confirm": "{0} herunterfahren",
  "vm.shutdown.body": "Das Gastsystem wird gebeten herunterzufahren, als würde der Einschaltknopf gedrückt.",
  "vm.state.running": "Läuft",
  "vm.state.shut off": "Aus",
  "vm.state.paused": "Pausiert",
  "vm.state.in shutdown": "Fährt herunter",
  "vm.state.crashed": "Abgestürzt",
  "vm.state.pmsuspended": "Ruhezustand",
  "vm.state.idle": "Leerlauf",
  "vm.state.unknown": "Unbekannt",
  "vm.more": "Weitere Aktionen",

  "empty.title": "Noch keine Maschinen",
  "empty.body": "Füge in den Einstellungen einen Server hinzu oder übernimm ihn aus deiner SSH-Konfiguration.",
  "empty.action": "Einstellungen öffnen",

  "vnc.connecting": "Verbinde mit {0}",
  "vnc.disconnected": "Getrennt",
  "vnc.reconnect": "Neu verbinden",
  "vnc.close": "Schließen",
  "vnc.cad": "Strg+Alt+Entf",
  "vnc.fit": "An Fenster anpassen",
  "vnc.actual": "Originalgröße",
  "vnc.fullscreen": "Vollbild",
  "vnc.paste": "Zwischenablage eintippen",
  "vnc.password": "VNC-Passwort",
  "vnc.password.body": "Der VNC-Server verlangt ein Passwort.",
  "vnc.connect": "Verbinden",
  "vnc.security": "VNC-Sicherheitsfehler: {0}",

  "hostkey.title": "Unbekannter Server",
  "hostkey.body": "DevBench war noch nie mit {0} verbunden. Vergleiche den Fingerabdruck mit dem Schlüssel des Servers, bevor du ihm vertraust, zum Beispiel mit ssh-keygen -lf /etc/ssh/ssh_host_ed25519_key.pub auf dem Server.",
  "hostkey.trust": "Vertrauen und verbinden",
  "hostkey.changed.title": "Host-Schlüssel geändert",

  "confirm.cancel": "Abbrechen",

  "settings.title": "Einstellungen",
  "settings.file": "Konfigurationsdatei",
  "settings.openFolder": "Im Ordner zeigen",
  "settings.reload": "Aus Datei neu laden",
  "settings.save": "Speichern",
  "settings.saved": "Einstellungen gespeichert",
  "settings.discard": "Änderungen verwerfen",
  "settings.unsaved": "Ungespeicherte Änderungen",
  "settings.language": "Sprache",
  "settings.language.auto": "System",
  "settings.addMachine": "Maschine hinzufügen",
  "settings.import": "Aus SSH-Konfiguration übernehmen",
  "settings.import.none": "Keine Hosts in ~/.ssh/config gefunden",
  "settings.import.added": "Hinzugefügt",
  "settings.removeMachine": "Maschine entfernen",
  "settings.removeVm": "VM entfernen",
  "settings.addVm": "VM hinzufügen",
  "settings.detectVms": "VMs auf dieser Maschine suchen",
  "settings.detect.none": "Keine neuen VMs gefunden",
  "settings.detect.save": "Speichere die Maschine zuerst",

  "field.name": "Name",
  "field.sshHost": "SSH-Host",
  "field.sshHost.hint": "Hostname, IP-Adresse oder ein Alias aus ~/.ssh/config",
  "field.sshPort": "Port",
  "field.sshUser": "Benutzer",
  "field.sshKey": "Privater Schlüssel",
  "field.jump": "Jump Host",
  "field.jump.hint": "benutzer@host:port oder ein Alias. Leer nimmt ProxyJump aus ~/.ssh/config",
  "field.fromSshConfig": "Aus SSH-Konfiguration",
  "field.mac": "MAC-Adresse",
  "field.mac.hint": "Für Wake-on-LAN. Leer blendet Aufwecken aus",
  "field.broadcast": "Broadcast-Adresse",
  "field.wolPort": "WoL-Port",
  "field.libvirtName": "libvirt-Name",
  "field.libvirtUri": "libvirt-URI",
  "field.guestAddress": "Gast-Adresse",
  "field.guestAddress.hint": "Leer fragt libvirt nach der IP des Gasts",
  "field.guestUser": "SSH-Benutzer im Gast",
  "field.guestPort": "SSH-Port im Gast",
  "field.guestKey": "Privater Schlüssel für den Gast",
  "field.sameAsMachine": "Wie bei der Maschine",
  "field.vncMode": "VNC",
  "field.vncMode.guest": "Über den Gast (SSH in die VM)",
  "field.vncMode.host": "Von der Maschine aus (z. B. QEMU-Anzeige)",
  "field.vncMode.none": "Aus",
  "field.vncTarget": "VNC-Server",
  "field.vncPort": "VNC-Port",
  "section.connection": "Verbindung",
  "section.wol": "Wake-on-LAN",
  "section.vms": "Virtuelle Maschinen",
  "section.agents": "KI-Agenten",
  "agents.hint": "Was Claude und andere Agenten über devbench mcp dürfen. Standardmäßig ist alles aus.",
  "agent.access": "Darf die Maschine sehen und aufwecken",
  "agent.exec": "Darf Befehle ausführen und Dateien bearbeiten",
  "agent.power": "Darf herunterfahren, neu starten und VMs hart ausschalten",
  "agent.vm.access": "Darf die VM starten und stoppen, den Bildschirm sehen und Maus und Tastatur benutzen",
  "agent.vm.exec": "Darf im Gast Befehle ausführen und Dateien bearbeiten",

  "activity.title": "Agenten-Aktivität",
  "activity.empty": "Noch kein Agent hat DevBench benutzt",
  "activity.hint": "Jede Aktion, die ein KI-Agent über devbench mcp ausführt, steht hier.",

  "update.available": "Version {0} ist verfügbar",
  "update.install": "Installieren und neu starten",
  "update.installing": "Wird installiert",

  "error.config": "Die Konfigurationsdatei konnte nicht gelesen werden",
};

const dicts: Record<Lang, Record<Key, string>> = { en, de };

function systemLang(): Lang {
  return navigator.language.toLowerCase().startsWith("de") ? "de" : "en";
}

let choice = $state<"auto" | Lang>((localStorage.getItem("devbench.lang") as "auto" | Lang | null) ?? "auto");

export function lang(): Lang {
  return choice === "auto" ? systemLang() : choice;
}

export function langChoice(): "auto" | Lang {
  return choice;
}

export function setLang(v: "auto" | Lang) {
  choice = v;
  localStorage.setItem("devbench.lang", v);
  document.documentElement.lang = lang();
}

export function t(key: Key, ...args: (string | number)[]): string {
  let s: string = dicts[lang()][key] ?? en[key] ?? key;
  args.forEach((a, i) => (s = s.replace(`{${i}}`, String(a))));
  return s;
}

export function vmStateLabel(state: string): string {
  const key = `vm.state.${state || "unknown"}` as Key;
  return key in en ? t(key) : state;
}
