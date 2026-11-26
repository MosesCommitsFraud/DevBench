# Wake-on-LAN einrichten

*English summary at the end.*

DevBench weckt einen ausgeschalteten Server mit einem „Magic Packet“: einem UDP-Broadcast an die MAC-Adresse der Netzwerkkarte. Damit das funktioniert, müssen drei Dinge stimmen: BIOS/UEFI, Linux auf dem Server und das Netzwerk.

## 1. BIOS / UEFI

Im BIOS/UEFI des Servers (Namen unterscheiden sich je nach Hersteller):

- **Wake on LAN**, **Power On By PCI-E / PCIe Devices**, **Resume by LAN** oder **Remote Wake Up**: einschalten.
- **ErP / EuP Ready** (oder „Deep Sleep“): **ausschalten**. Sonst bekommt die Netzwerkkarte im ausgeschalteten Zustand keinen Strom.

Test: Server herunterfahren und an der Netzwerkbuchse schauen – die Link-LED sollte weiter leuchten. Ist sie aus, hört die Karte keine Pakete.

## 2. Linux auf dem Server

Viele Treiber schalten WoL beim Herunterfahren ab. Erst den Namen der Netzwerkschnittstelle und die MAC-Adresse herausfinden:

```sh
ip -br link            # z. B. enp5s0  UP  aa:bb:cc:dd:ee:ff
sudo ethtool enp5s0 | grep -i wake
#   Supports Wake-on: pumbg
#   Wake-on: d          <- d = aus, g = Magic Packet
```

Steht bei `Supports Wake-on` kein `g`, kann die Karte kein Magic Packet.

Einmalig einschalten (gilt nur bis zum nächsten Neustart):

```sh
sudo ethtool -s enp5s0 wol g
```

**Dauerhaft**, je nachdem, was das Netzwerk verwaltet:

- **NetworkManager** (Desktop, oft auch Server):
  ```sh
  nmcli -t -f NAME,DEVICE c show --active     # Verbindungsnamen finden
  sudo nmcli c modify "<Verbindung>" 802-3-ethernet.wake-on-lan magic
  sudo nmcli c up "<Verbindung>"
  ```
- **netplan** (Standard bei Ubuntu Server): in `/etc/netplan/*.yaml` bei der Schnittstelle `wakeonlan: true` ergänzen, dann `sudo netplan apply`:
  ```yaml
  network:
    ethernets:
      enp5s0:
        wakeonlan: true
        # ... bisherige Einstellungen (dhcp4 usw.) bleiben
  ```
- **systemd-networkd ohne netplan:** Datei `/etc/systemd/network/50-wol.link` anlegen (die `Name`/`MAC`-Policy-Zeilen sind nötig, weil diese Datei die Standard-`.link`-Datei für die Karte ersetzt):
  ```ini
  [Match]
  MACAddress=aa:bb:cc:dd:ee:ff

  [Link]
  NamePolicy=kernel database onboard slot path
  MACAddressPolicy=persistent
  WakeOnLan=magic
  ```
  Bei netplan diese Datei *nicht* verwenden: netplan erzeugt eigene `10-netplan-*.link`-Dateien, die Vorrang haben.

Nach einem Neustart mit `sudo ethtool enp5s0 | grep Wake-on` prüfen, dass `Wake-on: g` steht.

**Bridge (z. B. `br0` für VMs):** WoL gehört auf die *physische* Karte (z. B. `enp5s0`), nicht auf die Bridge. Die MAC in DevBench ist die der physischen Karte.

## 3. Netzwerk und DevBench-Konfiguration

- Das Magic Packet ist ein **Broadcast im lokalen Netz**. Der PC mit DevBench und der Server müssen im selben LAN (derselben Broadcast-Domäne) sein. Über Internet oder VPN geht es nicht ohne ein Gerät im Server-LAN, das das Paket weiterleitet (Relay, z. B. Router mit WoL-Funktion).
- In DevBench beim Server eintragen:
  - `mac`: MAC der physischen Netzwerkkarte, z. B. `AA:BB:CC:DD:EE:FF`.
  - `wol_broadcast`: Broadcast-Adresse des Server-Netzes, z. B. `192.168.1.255` für `192.168.1.0/24`. `255.255.255.255` geht meistens auch, kann aber bei mehreren Netzwerkkarten am PC über die falsche hinausgehen.
  - `wol_port`: `9` (Standard) oder `7`.
- Windows-Firewall: ausgehende UDP-Broadcasts sind normalerweise erlaubt; nichts zu tun.

## Fehlersuche

- Funktioniert WoL nach einem *sauberen* Herunterfahren, aber nicht nach Stromausfall? Manche Boards brauchen nach Stromverlust einmal einen normalen Start. BIOS-Option **Restore on AC Power Loss** kann helfen.
- Mit `tcpdump` auf einem anderen Linux-Rechner im LAN prüfen, ob das Paket ankommt: `sudo tcpdump -i any -n udp port 9`.

---

## English summary

1. **BIOS/UEFI:** enable *Wake on LAN* / *Power On By PCI-E*, disable *ErP/EuP*. The link LED should stay on when the server is off.
2. **Linux:** `sudo ethtool -s <iface> wol g`, made persistent with NetworkManager (`nmcli c modify <conn> 802-3-ethernet.wake-on-lan magic`), netplan `wakeonlan: true`, or (networkd without netplan) a systemd `.link` file with `WakeOnLan=magic`. Use the physical NIC, not a bridge.
3. **Network:** the magic packet is a LAN broadcast; PC and server must share the LAN. Set `mac`, `wol_broadcast` (e.g. `192.168.1.255`) and `wol_port` (9) in DevBench.
