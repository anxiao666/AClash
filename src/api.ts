import { invoke } from "@tauri-apps/api/core";

export type AirportProfile = {
  id: string;
  name: string;
  url: string;
  fileName: string | null;
  enabled: boolean;
  updatedAt: string;
  nodeCount: number;
  upload: number;
  download: number;
  total: number;
  expire: number | null;
};

export type AppSettings = {
  airports: AirportProfile[];
  airportUrl: string;
  airportFileName: string | null;
  mixedPort: number;
  socksPort: number;
  httpPort: number;
  externalController: string;
  secret: string;
  allowLan: boolean;
  ipv6: boolean;
  bindAddress: string;
  tunEnabled: boolean;
  proxyEnabled: boolean;
  proxyMode: "system" | "tun" | string;
  lanDirect: boolean;
  companyDomains: string[];
  companyCidrsOverride: string[];
  companyImported: boolean;
  selectedProxy: string | null;
  policyDomestic: string;
  policyForeign: string;
  policyFinal: string;
  policyCompany: string;
  customRules: CustomRule[];
};

export type CustomRule = {
  id: string;
  ruleType: string;
  payload: string;
  target: string;
  enabled: boolean;
};

export type RuleListItem = {
  id: string | null;
  ruleType: string;
  payload: string;
  proxy: string;
  enabled: boolean;
  custom: boolean;
  /** custom | main | internal */
  group: string;
  size: number | null;
};

export type RulesPayload = {
  items: RuleListItem[];
  targets: string[];
};

export type CompanyMeta = {
  enabled: boolean;
  domains: string[];
  cidrs: string[];
  dnsServers: string[];
  endpoint: string;
  importedAt: string;
  conflictIps?: string[];
  name?: string;
  domain?: string;
  allowedIps?: string[];
  mtu?: number | null;
  persistentKeepalive?: number | null;
};

export type StatusPayload = {
  running: boolean;
  elevated: boolean;
  tunEnabled: boolean;
  proxyEnabled: boolean;
  proxyMode: "system" | "tun" | string;
  systemProxy: boolean;
  mixedPort: number;
  externalController: string;
  mixedPortBound: boolean;
  companyImported: boolean;
  companyEnabled?: boolean;
  airportConfigured: boolean;
  airportCount: number;
  selectedProxy: string | null;
  dataDir: string;
  version: { version?: string } | null;
  memoryBytes?: number | null;
};

export type SettingsPayload = {
  settings: AppSettings;
  company: CompanyMeta | null;
  elevated: boolean;
};

export type ProxyGroup = {
  name?: string;
  type?: string;
  now?: string;
  all?: string[];
  history?: Array<{ delay?: number }>;
};

export type DelayItem = {
  name: string;
  delay: number | null;
  error: string | null;
};

export async function getStatus() {
  return invoke<StatusPayload>("get_status");
}

export async function getSettings() {
  return invoke<SettingsPayload>("get_settings");
}

export async function saveSettings(patch: AppSettings) {
  return invoke<SettingsPayload>("save_settings", { patch });
}

export async function startCore() {
  return invoke<StatusPayload>("start_core");
}

export async function stopCore() {
  return invoke<StatusPayload>("stop_core");
}

export async function restartCore() {
  return invoke<StatusPayload>("restart_core");
}

export async function setTun(enabled: boolean) {
  return invoke<StatusPayload>("set_tun", { enabled });
}

export async function setProxyEnabled(enabled: boolean) {
  return invoke<StatusPayload>("set_proxy_enabled", { enabled });
}

export async function setProxyMode(mode: "system" | "tun") {
  return invoke<StatusPayload>("set_proxy_mode", { mode });
}

export async function addAirportUrl(url: string, name?: string) {
  return invoke<SettingsPayload>("add_airport_url", { url, name: name || null });
}

export async function addAirportFile(path: string, name?: string) {
  return invoke<SettingsPayload>("add_airport_file", { path, name: name || null });
}

export async function updateAirport(id?: string) {
  return invoke<SettingsPayload>("update_airport", { id: id || null });
}

export async function removeAirport(id: string) {
  return invoke<SettingsPayload>("remove_airport", { id });
}

export async function renameAirport(id: string, name: string) {
  return invoke<SettingsPayload>("rename_airport", { id, name });
}

export async function enableCompanyFromConf(confText: string) {
  return invoke<SettingsPayload>("enable_company_from_conf", { confText });
}

export async function setCompanyEnabled(enabled: boolean) {
  return invoke<SettingsPayload>("set_company_enabled", { enabled });
}

export async function clearCompanyWg() {
  return invoke<SettingsPayload>("clear_company_wg");
}

export async function getProxyGroup() {
  return invoke<ProxyGroup>("get_proxy_group");
}

export type AirportNodes = {
  id: string;
  nodes: string[];
};

export async function listAirportNodes() {
  return invoke<AirportNodes[]>("list_airport_nodes");
}

export async function getRules() {
  return invoke<RulesPayload>("get_rules");
}

export type PolicyGroupState = {
  name: string;
  now: string;
  all: string[];
};

export type PolicyGroupsPayload = {
  company: PolicyGroupState | null;
  domestic: PolicyGroupState;
  foreign: PolicyGroupState;
  finalGroup: PolicyGroupState;
};

export async function getPolicyGroups() {
  return invoke<PolicyGroupsPayload>("get_policy_groups");
}

export async function setPolicyGroup(group: string, selected: string) {
  return invoke<PolicyGroupsPayload>("set_policy_group", { group, selected });
}

export async function addCustomRule(input: {
  ruleType: string;
  payload: string;
  target: string;
  enabled?: boolean;
}) {
  return invoke<RulesPayload>("add_custom_rule", { input });
}

export async function setCustomRuleEnabled(id: string, enabled: boolean) {
  return invoke<RulesPayload>("set_custom_rule_enabled", { id, enabled });
}

export async function removeCustomRule(id: string) {
  return invoke<RulesPayload>("remove_custom_rule", { id });
}

export async function selectProxyNode(name: string) {
  return invoke<void>("select_proxy_node", { name });
}

export async function testProxyDelay(name: string, url?: string) {
  return invoke<number>("test_proxy_delay", { name, url: url || null });
}

export type IpItem = {
  key: string;
  label: string;
  hint: string;
  value: string;
};

export type OutboundIpPayload = {
  items: IpItem[];
  viaProxy: boolean;
};

export async function getOutboundIp() {
  return invoke<OutboundIpPayload>("get_outbound_ip");
}

export async function testProxyDelays(names: string[]) {
  return invoke<DelayItem[]>("test_proxy_delays", { names });
}

export async function getConnections() {
  return invoke<{
    connections?: unknown[];
    uploadTotal?: number;
    downloadTotal?: number;
  }>("get_connections");
}

export async function readLogs(maxLines = 400) {
  return invoke<string>("read_logs", { maxLines });
}

export async function clearLogs() {
  return invoke<void>("clear_logs");
}

export async function openDataDir() {
  return invoke<void>("open_data_dir");
}

export async function readTextFile(path: string) {
  return invoke<string>("read_text_file", { path });
}

export async function quitApp() {
  return invoke<void>("quit_app");
}

export async function getAutostart() {
  return invoke<boolean>("get_autostart");
}

export async function setAutostart(enabled: boolean) {
  return invoke<boolean>("set_autostart", { enabled });
}
