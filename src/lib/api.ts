// Typed wrappers around the Rust commands (src-tauri/src/lib.rs).
import { invoke } from "@tauri-apps/api/core";

export type VncMode = "guest" | "host" | "none";

export interface Vm {
  id: string;
  name: string;
  libvirt_name: string;
  libvirt_uri: string;
  guest_address: string;
  guest_ssh_user: string;
  guest_ssh_port: number;
  guest_ssh_key: string;
  vnc_mode: VncMode;
  vnc_host: string;
  vnc_port: number;
  agent_access: boolean;
  agent_exec: boolean;
}

export interface Machine {
  id: string;
  name: string;
  ssh_host: string;
  ssh_port: number;
  ssh_user: string;
  ssh_key: string;
  jump_host: string;
  mac: string;
  wol_broadcast: string;
  wol_port: number;
  agent_access: boolean;
  agent_exec: boolean;
  agent_power: boolean;
  vms: Vm[];
}

export interface Config {
  machines: Machine[];
}

export interface VmStatus {
  id: string;
  state: string;
  running: boolean;
  error: string | null;
}

export interface MachineStatus {
  online: boolean;
  hostname: string;
  kernel: string;
  uptime_secs: number;
  load: [number, number, number];
  cpus: number;
  cpu_percent: number;
  mem_total: number;
  mem_available: number;
  disk_total: number;
  disk_used: number;
  vms: VmStatus[];
}

export interface SshHostEntry {
  alias: string;
  host_name: string;
  user: string;
  port: number;
  proxy_jump: string;
}

export interface ActivityEntry {
  time: string;
  tool: string;
  machine: string;
  vm: string;
  summary: string;
  ok: boolean;
  error: string;
}

export type AppError =
  | { kind: "error"; message: string }
  | { kind: "host_key_unknown"; message: string; host: string; port: number; fingerprint: string; key: string }
  | { kind: "host_key_changed"; message: string; host: string; port: number; fingerprint: string };

export function asAppError(e: unknown): AppError {
  if (e && typeof e === "object" && "kind" in e) return e as AppError;
  return { kind: "error", message: String(e) };
}

export const api = {
  getConfig: () => invoke<Config>("get_config"),
  startupError: () => invoke<string | null>("startup_error"),
  saveConfig: (config: Config) => invoke<Config>("save_config", { config }),
  reloadConfig: () => invoke<Config>("reload_config"),
  configPath: () => invoke<string>("config_path"),
  sshConfigHosts: () => invoke<SshHostEntry[]>("ssh_config_hosts"),
  machineStatus: (machineId: string) => invoke<MachineStatus>("machine_status", { machineId }),
  wake: (machineId: string) => invoke<void>("wake", { machineId }),
  power: (machineId: string, action: "poweroff" | "reboot") => invoke<void>("power", { machineId, action }),
  vmAction: (machineId: string, vmId: string, action: "start" | "shutdown" | "reboot" | "destroy") =>
    invoke<void>("vm_action", { machineId, vmId, action }),
  listDomains: (machineId: string, uri: string) => invoke<string[]>("list_domains", { machineId, uri }),
  trustHostKey: (host: string, port: number, key: string) => invoke<void>("trust_host_key", { host, port, key }),
  vncOpen: (machineId: string, vmId: string) => invoke<{ url: string; title: string }>("vnc_open", { machineId, vmId }),
  activity: (limit: number) => invoke<ActivityEntry[]>("activity", { limit }),
};

export function newId(): string {
  const b = new Uint8Array(6);
  crypto.getRandomValues(b);
  return Array.from(b, (x) => x.toString(16).padStart(2, "0")).join("");
}

export function emptyMachine(): Machine {
  return {
    id: newId(),
    name: "",
    ssh_host: "",
    ssh_port: 0,
    ssh_user: "",
    ssh_key: "",
    jump_host: "",
    mac: "",
    wol_broadcast: "255.255.255.255",
    wol_port: 9,
    agent_access: false,
    agent_exec: false,
    agent_power: false,
    vms: [],
  };
}

export function emptyVm(libvirtName = ""): Vm {
  return {
    id: newId(),
    name: libvirtName,
    libvirt_name: libvirtName,
    libvirt_uri: "qemu:///system",
    guest_address: "",
    guest_ssh_user: "",
    guest_ssh_port: 22,
    guest_ssh_key: "",
    vnc_mode: "guest",
    vnc_host: "127.0.0.1",
    vnc_port: 5900,
    agent_access: false,
    agent_exec: false,
  };
}
