import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import { open } from "@tauri-apps/plugin-dialog";
import "./App.css";
import powerBud from "./assets/power-bud.png";
import { flagSrcForCode, regionCodeForNodeName } from "./regionFlag";
import {
  addAirportFile,
  addAirportUrl,
  clearCompanyWg,
  clearLogs,
  getAutostart,
  getConnections,
  getProxyGroup,
  getSettings,
  getStatus,
  getOutboundIp,
  getPolicyGroups,
  getRules,
  IpItem,
  listAirportNodes,
  enableCompanyFromConf,
  setCompanyEnabled,
  PolicyGroupsPayload,
  ProxyGroup,
  quitApp,
  readLogs,
  readTextFile,
  removeAirport,
  removeCustomRule,
  renameAirport,
  restartCore,
  RuleListItem,
  saveSettings,
  selectProxyNode,
  setAutostart,
  setCustomRuleEnabled,
  setPolicyGroup,
  setProxyEnabled,
  setProxyMode,
  addCustomRule,
  AppSettings,
  SettingsPayload,
  StatusPayload,
  testProxyDelay,
  testProxyDelays,
  updateAirport,
} from "./api";

function formatMemory(n?: number | null) {
  if (!n || n <= 0) return "-";
  if (n < 1024 * 1024) return `${(n / 1024).toFixed(1)} KB`;
  return `${(n / 1024 / 1024).toFixed(1)} MB`;
}

function shortIp(value: string) {
  const v = (value || "").trim();
  if (!v || v === "—" || v === "-") return "—";
  if (v.length <= 16) return v;
  return `${v.slice(0, 8)}…${v.slice(-6)}`;
}

function ipChipLabel(label: string) {
  return label.replace(/\s*IP$/, "").replace(/出口$/, "") || label;
}

function RefreshIcon() {
  return (
    <svg className="refresh-icon" viewBox="0 0 24 24" fill="none" aria-hidden="true">
      <path
        d="M20 12a8 8 0 1 1-2.34-5.66"
        stroke="currentColor"
        strokeWidth="1.8"
        strokeLinecap="round"
      />
      <path
        d="M20 5v5h-5"
        stroke="currentColor"
        strokeWidth="1.8"
        strokeLinecap="round"
        strokeLinejoin="round"
      />
    </svg>
  );
}

function TrashIcon() {
  return (
    <svg className="trash-icon" viewBox="0 0 24 24" fill="none" aria-hidden="true">
      <path
        d="M4 7h16"
        stroke="currentColor"
        strokeWidth="1.8"
        strokeLinecap="round"
      />
      <path
        d="M9 7V5a1 1 0 0 1 1-1h4a1 1 0 0 1 1 1v2"
        stroke="currentColor"
        strokeWidth="1.8"
        strokeLinecap="round"
        strokeLinejoin="round"
      />
      <path
        d="M6.5 7l.8 12a1.5 1.5 0 0 0 1.5 1.4h6.4a1.5 1.5 0 0 0 1.5-1.4l.8-12"
        stroke="currentColor"
        strokeWidth="1.8"
        strokeLinecap="round"
        strokeLinejoin="round"
      />
      <path
        d="M10 11v6M14 11v6"
        stroke="currentColor"
        strokeWidth="1.8"
        strokeLinecap="round"
      />
    </svg>
  );
}

type Tab = "home" | "nodes" | "logs" | "settings" | "kernel" | "rules";

function controllerPort(ctrl: string | undefined | null) {
  const raw = (ctrl || "127.0.0.1:9090").trim();
  const part = raw.includes(":") ? raw.split(":").pop() : raw;
  const n = Number(part);
  return Number.isFinite(n) && n > 0 && n < 65536 ? n : 9090;
}

function randomPort() {
  return 10000 + Math.floor(Math.random() * 50000);
}

function portText(n: number) {
  return n > 0 ? String(n) : "";
}

/** Keep empty while editing; reject non-digits so the field can be cleared. */
function onPortTyping(raw: string) {
  if (raw === "") return "";
  if (!/^\d{1,5}$/.test(raw)) return null;
  return raw;
}

function parsePortInput(raw: string, label: string) {
  const t = raw.trim();
  if (!t) throw new Error(`${label}不能为空`);
  const n = Number(t);
  if (!Number.isInteger(n) || n < 1 || n > 65535) {
    throw new Error(`${label}需在 1–65535 之间`);
  }
  return n;
}

function newSecret() {
  const bytes = new Uint8Array(16);
  crypto.getRandomValues(bytes);
  return Array.from(bytes, (b) => b.toString(16).padStart(2, "0")).join("");
}

type LogEntry = {
  id: string;
  level: string;
  time: string;
  body: string;
};

function formatBytes(n = 0) {
  if (n < 1024) return `${n} B`;
  if (n < 1024 * 1024) return `${(n / 1024).toFixed(1)} KB`;
  if (n < 1024 * 1024 * 1024) return `${(n / 1024 / 1024).toFixed(1)} MB`;
  if (n < 1024 * 1024 * 1024 * 1024) return `${(n / 1024 / 1024 / 1024).toFixed(2)} GB`;
  return `${(n / 1024 / 1024 / 1024 / 1024).toFixed(2)} TB`;
}

function formatRate(n = 0) {
  if (n < 1024) return `${n.toFixed(0)} B/s`;
  if (n < 1024 * 1024) return `${(n / 1024).toFixed(1)} KB/s`;
  return `${(n / 1024 / 1024).toFixed(2)} MB/s`;
}

function formatExpire(expire: number | null | undefined) {
  if (!expire) return "-";
  const d = new Date(expire * 1000);
  if (Number.isNaN(d.getTime())) return "-";
  return d.toISOString().slice(0, 10);
}

function errMessage(e: unknown) {
  if (typeof e === "string") return e;
  if (e && typeof e === "object" && "message" in e) {
    return String((e as { message: unknown }).message);
  }
  return String(e);
}

function formatLogTime(raw: string) {
  if (!raw) return "";
  const d = new Date(raw.includes("T") ? raw : raw.replace(" ", "T"));
  if (Number.isNaN(d.getTime())) return raw;
  return d.toLocaleString();
}

function parseLogLines(raw: string): LogEntry[] {
  const lines = raw
    .split(/\r?\n/)
    .map((s) => s.trim())
    .filter(Boolean)
    .slice(-200)
    .reverse();
  return lines.map((line, idx) => {
    let level = "INFO";
    let time = "";
    let body = line;

    const kvLevel = line.match(/\blevel=([a-zA-Z]+)\b/);
    const kvTime = line.match(/\btime="([^"]+)"/);
    const kvMsg = line.match(/\bmsg="((?:\\.|[^"\\])*)"/);
    if (kvLevel || kvTime || kvMsg) {
      level = (kvLevel?.[1] || "info").toUpperCase();
      if (level === "WARNING") level = "WARN";
      time = formatLogTime(kvTime?.[1] || "");
      body = kvMsg ? kvMsg[1].replace(/\\"/g, '"').replace(/\\n/g, "\n") : line;
    } else {
      const levelMatch = line.match(/\b(INFO|WARN|WARNING|ERROR|DEBUG|TRACE)\b/i);
      const timeMatch = line.match(
        /(\d{4}[-\/]\d{2}[-\/]\d{2}[ T]\d{2}:\d{2}:\d{2}(?:\.\d+)?(?:Z|[+-]\d{2}:?\d{2})?)/,
      );
      level = (levelMatch?.[1] || "INFO").toUpperCase();
      if (level === "WARNING") level = "WARN";
      time = formatLogTime(timeMatch?.[1] || "");
      body = line
        .replace(/\btime="[^"]*"\s*/g, "")
        .replace(/\blevel=[a-zA-Z]+\s*/g, "")
        .replace(/\bmsg="/, "")
        .replace(/"\s*$/, "")
        .replace(/^\[[^\]]*]\s*/, "")
        .replace(/^(INFO|WARN|WARNING|ERROR|DEBUG|TRACE)\s*/i, "")
        .replace(
          /^\d{4}[-\/]\d{2}[-\/]\d{2}[ T]\d{2}:\d{2}:\d{2}(?:\.\d+)?(?:Z|[+-]\d{2}:?\d{2})?\s*/,
          "",
        )
        .trim();
      if (!body) body = line;
    }

    return {
      id: `${idx}-${line.slice(0, 24)}`,
      level,
      time,
      body,
    };
  });
}

function NodeFlag({ name }: { name: string }) {
  const code = regionCodeForNodeName(name);
  const src = flagSrcForCode(code);
  if (!src) return null;
  return (
    <img
      className="node-flag"
      src={src}
      alt=""
      width={18}
      height={13}
      onError={(e) => {
        (e.currentTarget as HTMLImageElement).style.display = "none";
      }}
    />
  );
}

function PowerIcon() {
  return <img className="power-icon" src={powerBud} alt="" width={32} height={32} />;
}

function LogoMark() {
  return (
    <div className="logo-mark" aria-hidden="true">
      <svg viewBox="0 0 48 48" fill="none">
        <path
          d="M24 6 11.8 30.4c-1 2 .3 4.4 2.6 4.6 1.8.2 3.4-.8 4.1-2.4L24 17.8"
          fill="#556b3a"
        />
        <path
          d="M24 6 36.2 30.4c1 2-.3 4.4-2.6 4.6-1.8.2-3.4-.8-4.1-2.4L24 17.8"
          fill="#6f8f4e"
        />
        <path
          d="M14.2 31.2c4.2-2.4 8.2-5.8 11.2-9.4 1.1-1.3 3-1.4 4.3-.2 1.3 1.2 1.3 3.1.1 4.4-3.7 3.9-8.5 7-13.5 8.1-1.7.4-2.8-1-2.1-2.9Z"
          fill="#e0d09a"
        />
      </svg>
    </div>
  );
}

function VineBanner() {
  return (
    <div className="vine" aria-hidden="true">
      <svg viewBox="0 0 360 36" preserveAspectRatio="xMidYMid meet">
        <path
          className="vine-stem"
          d="M4 22 C28 10, 48 28, 72 18 S116 8, 140 20 188 30, 212 16 260 6, 284 18 332 28, 356 14"
        />
        <g className="vine-leaves">
          <ellipse cx="28" cy="12" rx="9" ry="5" transform="rotate(-28 28 12)" />
          <ellipse cx="56" cy="24" rx="8" ry="4.5" transform="rotate(22 56 24)" />
          <ellipse cx="92" cy="14" rx="10" ry="5.2" transform="rotate(-18 92 14)" />
          <ellipse cx="124" cy="26" rx="8.5" ry="4.6" transform="rotate(26 124 26)" />
          <ellipse cx="164" cy="12" rx="9.5" ry="5" transform="rotate(-24 164 12)" />
          <ellipse cx="196" cy="24" rx="8" ry="4.4" transform="rotate(20 196 24)" />
          <ellipse cx="236" cy="11" rx="10" ry="5.2" transform="rotate(-16 236 11)" />
          <ellipse cx="268" cy="24" rx="8.5" ry="4.6" transform="rotate(24 268 24)" />
          <ellipse cx="308" cy="14" rx="9" ry="4.8" transform="rotate(-22 308 14)" />
          <ellipse cx="340" cy="22" rx="8" ry="4.4" transform="rotate(18 340 22)" />
        </g>
        <g className="vine-buds">
          <circle cx="74" cy="20" r="2.2" />
          <circle cx="148" cy="18" r="2" />
          <circle cx="220" cy="17" r="2.2" />
          <circle cx="292" cy="19" r="2" />
        </g>
      </svg>
    </div>
  );
}

function App() {
  const [tab, setTab] = useState<Tab>("home");
  const [status, setStatus] = useState<StatusPayload | null>(null);
  const [settingsPayload, setSettingsPayload] = useState<SettingsPayload | null>(null);
  const [proxyGroup, setProxyGroup] = useState<ProxyGroup | null>(null);
  const [delays, setDelays] = useState<Record<string, number | "bad">>({});
  const [airportNodes, setAirportNodes] = useState<Record<string, string[]>>({});
  const [proxyExpanded, setProxyExpanded] = useState<Record<string, boolean>>({});
  const [airportUrl, setAirportUrl] = useState("");
  const [importModalOpen, setImportModalOpen] = useState(false);
  const [manageAirport, setManageAirport] = useState<{
    id: string;
    name: string;
    url: string;
  } | null>(null);
  const [companyModalOpen, setCompanyModalOpen] = useState(false);
  const [renameModal, setRenameModal] = useState<{ id: string; name: string } | null>(
    null,
  );
  const [renameValue, setRenameValue] = useState("");
  const [ruleItems, setRuleItems] = useState<RuleListItem[]>([]);
  const [ruleTargets, setRuleTargets] = useState<string[]>(["DIRECT", "PROXY", "REJECT"]);
  const [ruleFilter, setRuleFilter] = useState("");
  const [ruleModalOpen, setRuleModalOpen] = useState(false);
  const [showInternalRules, setShowInternalRules] = useState(false);
  const [policyGroups, setPolicyGroups] = useState<PolicyGroupsPayload | null>(null);
  const [ruleDraft, setRuleDraft] = useState({
    ruleType: "DOMAIN-SUFFIX",
    payload: "",
    target: "PROXY",
  });
  const [logFilter, setLogFilter] = useState("");
  const [logLevel, setLogLevel] = useState("ALL");
  const [logEntries, setLogEntries] = useState<LogEntry[]>([]);
  const [autostart, setAutostartState] = useState(false);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [notice, setNotice] = useState<string | null>(null);
  const [upRate, setUpRate] = useState(0);
  const [downRate, setDownRate] = useState(0);
  const [airportCheck, setAirportCheck] = useState<"idle" | "ok" | "bad">("idle");
  const [companyCheck, setCompanyCheck] = useState<"idle" | "ok" | "bad">("idle");
  const [airportCheckText, setAirportCheckText] = useState<string | null>(null);
  const [companyCheckText, setCompanyCheckText] = useState<string | null>(null);
  const [checking, setChecking] = useState(false);
  const [ipItems, setIpItems] = useState<IpItem[]>([]);
  const [ipLoading, setIpLoading] = useState(false);
  const [ipModalOpen, setIpModalOpen] = useState(false);
  const [kernelDraft, setKernelDraft] = useState({
    mixedPort: "7890",
    socksPort: "",
    socksOn: false,
    httpPort: "",
    httpOn: false,
    controllerPort: "9090",
    secret: "",
    bindAddress: "",
    allowLan: false,
    ipv6: false,
  });
  const [secretVisible, setSecretVisible] = useState(false);
  const trafficRef = useRef<{ up: number; down: number; at: number } | null>(null);
  const lastTotals = useRef<{ up: number; down: number }>({ up: 0, down: 0 });

  const airports = settingsPayload?.settings.airports || [];

  const run = useCallback(async (fn: () => Promise<void>) => {
    setBusy(true);
    setError(null);
    setNotice(null);
    try {
      await fn();
    } catch (e) {
      setError(errMessage(e));
    } finally {
      setBusy(false);
    }
  }, []);

  const refreshStatus = useCallback(async () => {
    const s = await getStatus();
    setStatus(s);
    return s;
  }, []);

  const syncKernelDraft = useCallback((s: AppSettings) => {
    setKernelDraft({
      mixedPort: portText(s.mixedPort || 7890),
      socksPort: portText(s.socksPort > 0 ? s.socksPort : 7891),
      socksOn: (s.socksPort || 0) > 0,
      httpPort: portText(s.httpPort > 0 ? s.httpPort : 7892),
      httpOn: (s.httpPort || 0) > 0,
      controllerPort: portText(controllerPort(s.externalController)),
      secret: s.secret || "",
      bindAddress: s.bindAddress || "",
      allowLan: !!s.allowLan,
      ipv6: !!s.ipv6,
    });
  }, []);

  const refreshSettings = useCallback(async () => {
    const payload = await getSettings();
    setSettingsPayload(payload);
    syncKernelDraft(payload.settings);
    try {
      setPolicyGroups(await getPolicyGroups());
    } catch {
      /* core may be down */
    }
    return payload;
  }, [syncKernelDraft]);

  const refreshProxies = useCallback(async () => {
    try {
      setProxyGroup(await getProxyGroup());
    } catch {
      setProxyGroup(null);
    }
  }, []);

  const refreshAirportNodes = useCallback(async () => {
    try {
      const list = await listAirportNodes();
      const map: Record<string, string[]> = {};
      for (const item of list) map[item.id] = item.nodes || [];
      setAirportNodes(map);
    } catch {
      setAirportNodes({});
    }
  }, []);

  const refreshOutboundIp = useCallback(async () => {
    setIpLoading(true);
    try {
      const res = await getOutboundIp();
      setIpItems(res.items || []);
    } catch {
      setIpItems([]);
    } finally {
      setIpLoading(false);
    }
  }, []);

  const refreshRules = useCallback(async () => {
    try {
      const res = await getRules();
      setRuleItems(res.items || []);
      if (res.targets?.length) setRuleTargets(res.targets);
    } catch {
      setRuleItems([]);
    }
    try {
      setPolicyGroups(await getPolicyGroups());
    } catch {
      setPolicyGroups(null);
    }
  }, []);

  const refreshLogs = useCallback(async () => {
    try {
      setLogEntries(parseLogLines(await readLogs(500)));
    } catch {
      setLogEntries([]);
    }
  }, []);

  const applySettingsPayload = async (payload: SettingsPayload) => {
    setSettingsPayload(payload);
    syncKernelDraft(payload.settings);
    setStatus(await refreshStatus());
    await refreshProxies();
    await refreshAirportNodes();
  };

  useEffect(() => {
    const unlisten = listen<string>("tray-message", (event) => {
      const msg = (event.payload || "").trim();
      if (!msg) return;
      if (msg.includes("失败")) {
        setError(msg);
        setNotice(null);
      } else {
        setError(null);
        setNotice(msg);
      }
      void refreshStatus();
      void refreshSettings();
      void refreshProxies();
    });
    return () => {
      void unlisten.then((off) => off());
    };
  }, [refreshProxies, refreshSettings, refreshStatus]);

  useEffect(() => {
    void run(async () => {
      await refreshStatus();
      await refreshSettings();
      await refreshProxies();
      await refreshAirportNodes();
      await refreshLogs();
      setAutostartState(await getAutostart());
    });
  }, [
    refreshAirportNodes,
    refreshLogs,
    refreshProxies,
    refreshSettings,
    refreshStatus,
    run,
  ]);

  useEffect(() => {
    setProxyExpanded((prev) => {
      const next = { ...prev };
      let changed = false;
      for (const item of airports) {
        if (next[item.id] === undefined) {
          next[item.id] = false;
          changed = true;
        }
      }
      return changed ? next : prev;
    });
  }, [airports]);

  useEffect(() => {
    if (tab === "nodes") void refreshAirportNodes();
  }, [tab, refreshAirportNodes, airports.length]);

  useEffect(() => {
    const timer = window.setInterval(() => {
      void refreshStatus();
      void refreshProxies();
      if (tab === "logs") void refreshLogs();
      if (tab === "rules") void refreshRules();
    }, 2000);
    return () => window.clearInterval(timer);
  }, [refreshLogs, refreshProxies, refreshRules, refreshStatus, tab]);

  useEffect(() => {
    if (tab === "rules") void refreshRules();
  }, [tab, refreshRules, status?.running, settingsPayload?.company]);

  useEffect(() => {
    void refreshOutboundIp();
  }, [
    status?.running,
    status?.proxyEnabled,
    status?.proxyMode,
    status?.selectedProxy,
    refreshOutboundIp,
  ]);

  useEffect(() => {
    const name = (status?.selectedProxy || proxyGroup?.now || "").trim();
    if (!status?.running || !name || name === "DIRECT") return;
    let cancelled = false;
    void (async () => {
      try {
        const ms = await testProxyDelay(name);
        if (!cancelled) setDelays((prev) => ({ ...prev, [name]: ms }));
      } catch {
        if (!cancelled) setDelays((prev) => ({ ...prev, [name]: "bad" }));
      }
    })();
    return () => {
      cancelled = true;
    };
  }, [status?.running, status?.selectedProxy, proxyGroup?.now]);

  useEffect(() => {
    if (!status?.running) {
      setUpRate(0);
      setDownRate(0);
      return;
    }
    const timer = window.setInterval(() => {
      void (async () => {
        try {
          const data = await getConnections();
          const now = Date.now();
          const up = data.uploadTotal || 0;
          const down = data.downloadTotal || 0;
          lastTotals.current = { up, down };
          const prev = trafficRef.current;
          if (prev) {
            const dt = Math.max((now - prev.at) / 1000, 0.2);
            setUpRate(Math.max(0, (up - prev.up) / dt));
            setDownRate(Math.max(0, (down - prev.down) / dt));
          }
          trafficRef.current = { up, down, at: now };
        } catch {
          /* ignore */
        }
      })();
    }, 2000);
    return () => window.clearInterval(timer);
  }, [status?.running]);

  const kernelVersion = useMemo(() => {
    const raw = status?.version?.version?.trim();
    if (!raw) return status?.running ? "—" : "未运行";
    return /^v/i.test(raw) ? raw : `v${raw}`;
  }, [status]);

  const currentNode = status?.selectedProxy || proxyGroup?.now || "未选择节点";
  const proxyOn = !!status?.proxyEnabled;
  const proxyMode = status?.proxyMode === "tun" ? "tun" : "system";
  const statusLine = !status?.running
    ? "内核启动中…"
    : !proxyOn
      ? "代理已关闭 · 内核运行中"
      : proxyMode === "tun"
        ? "代理已开启 · TUN"
        : "代理已开启 · 系统代理";

  const filteredLogs = logEntries.filter((item) => {
    if (logLevel !== "ALL" && item.level !== logLevel) return false;
    if (!logFilter.trim()) return true;
    return item.body.toLowerCase().includes(logFilter.trim().toLowerCase());
  });

  return (
    <div className="app">
      <VineBanner />

      {tab === "home" && (
        <div className="shell">
          <header className="hero">
            <div className="hero-top">
              <div className="brand-block">
                <LogoMark />
                <div className="brand-text">
                  <h1>AClash</h1>
                  <p>{statusLine}</p>
                </div>
              </div>
              <button
                className={`power ${proxyOn ? "on" : ""}`}
                disabled={busy}
                title={proxyOn ? "关闭代理" : "开启代理"}
                onClick={() =>
                  void run(async () => {
                    setStatus(await setProxyEnabled(!proxyOn));
                  })
                }
              >
                <PowerIcon />
              </button>
            </div>
            <div className="hero-controls">
              <div className="seg compact">
                <button
                  className={proxyMode === "tun" ? "active" : ""}
                  disabled={busy}
                  onClick={() =>
                    void run(async () => {
                      setStatus(await setProxyMode("tun"));
                    })
                  }
                >
                  TUN
                </button>
                <button
                  className={proxyMode === "system" ? "active" : ""}
                  disabled={busy}
                  onClick={() =>
                    void run(async () => {
                      setStatus(await setProxyMode("system"));
                    })
                  }
                >
                  系统代理
                </button>
              </div>
            </div>
          </header>
          {error ? <div className="error">{error}</div> : null}
          {notice ? <div className="notice">{notice}</div> : null}
        </div>
      )}

      {tab !== "home" && error ? <div className="error">{error}</div> : null}
      {tab !== "home" && notice ? <div className="notice">{notice}</div> : null}

      <main className="content">
        {tab === "home" && (
          <div className="page page-home">
            <div className="page-body stack">
            <div className="speed-bar">
              <div className="speed-cell">
                <div className="label">
                  <span className="arrow up">↑</span>
                  <span>上行</span>
                </div>
                <div className="value">{status?.running ? formatRate(upRate) : "—"}</div>
              </div>
              <div className="speed-divider" />
              <div className="speed-cell">
                <div className="label">
                  <span className="arrow down">↓</span>
                  <span>下行</span>
                </div>
                <div className="value">{status?.running ? formatRate(downRate) : "—"}</div>
              </div>
            </div>

            <button className="policy" type="button" onClick={() => setTab("nodes")}>
              <div>
                <div className="title">当前节点</div>
                <div className="desc">
                  <NodeFlag name={currentNode} />
                  {currentNode} ·{" "}
                  {(() => {
                    const delay = delays[currentNode];
                    if (delay === "bad") return "超时";
                    if (typeof delay === "number") return `${delay} ms`;
                    return "未测速";
                  })()}
                </div>
              </div>
              <span className="chip">{proxyOn ? "代理中" : "未开启"}</span>
            </button>

            <button className="policy" type="button" onClick={() => setTab("rules")}>
              <div>
                <div className="title">分流规则</div>
                <div className="desc">
                  {policyGroups?.company
                    ? `公司 ${policyGroups.company.now} · `
                    : ""}
                  国内 {policyGroups?.domestic.now || settingsPayload?.settings.policyDomestic || "DIRECT"}
                  {" · "}
                  国外 {policyGroups?.foreign.now || settingsPayload?.settings.policyForeign || "PROXY"}
                  {" · "}
                  Final {policyGroups?.finalGroup.now || settingsPayload?.settings.policyFinal || "PROXY"}
                </div>
              </div>
              <span className="chip">策略</span>
            </button>

            <section className="card soft status-overview">
              <div className="status-head">
                <div className="title">状态总览</div>
                <button
                  className="ghost linkish"
                  disabled={busy || checking || !status?.running}
                  type="button"
                  onClick={() =>
                    void run(async () => {
                      setChecking(true);
                      try {
                        if (status?.airportConfigured) {
                          const node =
                            status.selectedProxy ||
                            proxyGroup?.now ||
                            (proxyGroup?.all || []).find((n) => n !== "DIRECT");
                          if (!node) {
                            setAirportCheck("bad");
                            setAirportCheckText("无可用节点");
                          } else {
                            try {
                              const ms = await testProxyDelay(node);
                              setAirportCheck("ok");
                              setAirportCheckText(`${ms} ms`);
                            } catch {
                              setAirportCheck("bad");
                              setAirportCheckText("异常");
                            }
                          }
                        } else {
                          setAirportCheck("idle");
                          setAirportCheckText(null);
                        }

                        if (settingsPayload?.company?.enabled) {
                          const companyName =
                            settingsPayload.company.name || "公司VPN";
                          const domain =
                            settingsPayload.company.domain ||
                            settingsPayload.company.domains?.[0] ||
                            "";
                          const policyNow =
                            policyGroups?.company?.now ||
                            settingsPayload.settings.policyCompany ||
                            companyName;
                          try {
                            if (policyNow === "DIRECT") {
                              setCompanyCheck("ok");
                              setCompanyCheckText("策略·直连");
                            } else if (policyNow === "PROXY") {
                              const node =
                                status?.selectedProxy ||
                                proxyGroup?.now ||
                                companyName;
                              const ms = await testProxyDelay(node);
                              setCompanyCheck("ok");
                              setCompanyCheckText(`策略·代理 ${ms} ms`);
                            } else {
                              const ms = await testProxyDelay(
                                policyNow || companyName,
                                domain ? `https://navigator.${domain}` : undefined,
                              );
                              setCompanyCheck("ok");
                              setCompanyCheckText(`${ms} ms`);
                            }
                          } catch {
                            setCompanyCheck("bad");
                            setCompanyCheckText("异常");
                          }
                        } else {
                          setCompanyCheck("idle");
                          setCompanyCheckText(null);
                        }
                      } finally {
                        setChecking(false);
                      }
                    })
                  }
                >
                  {checking ? "检测中…" : "检测"}
                </button>
              </div>
              <div className="meta-grid">
                <div
                  className={`meta-item ${
                    airportCheck === "ok"
                      ? "ok"
                      : airportCheck === "bad"
                        ? "bad"
                        : ""
                  }`}
                >
                  <div className="k">机场</div>
                  <div className="v">
                    {airportCheckText
                      ? airportCheckText
                      : status?.airportCount
                        ? `${status.airportCount} 个 · 未检测`
                        : "未导入"}
                  </div>
                </div>
                <div
                  className={`meta-item ${
                    companyCheck === "ok"
                      ? "ok"
                      : companyCheck === "bad"
                        ? "bad"
                        : ""
                  }`}
                >
                  <div className="k">公司 VPN</div>
                  <div className="v">
                    {companyCheckText
                      ? companyCheckText
                      : settingsPayload?.company?.enabled
                        ? "已启用 · 未检测"
                        : settingsPayload?.company
                          ? "已关闭"
                          : "未导入"}
                  </div>
                </div>
              </div>
            </section>

            <div className="card soft ip-card">
              <div className="status-head">
                <div className="title">当前 IP</div>
                <button
                  className="ghost linkish"
                  type="button"
                  onClick={() => {
                    setIpModalOpen(true);
                    if (!ipItems.length && !ipLoading) void refreshOutboundIp();
                  }}
                >
                  详情
                </button>
              </div>
              {status?.running && status.mixedPortBound === false && (
                <div className="ip-warn">
                  <div>
                    端口 {status.mixedPort} 未被当前内核占用（可能被其他代理软件占用）。
                    此时切节点不会改变真实出口。
                  </div>
                  <button
                    className="secondary"
                    type="button"
                    disabled={busy}
                    onClick={() =>
                      void run(async () => {
                        setStatus(await restartCore());
                        await refreshSettings();
                        await refreshProxies();
                        await refreshOutboundIp();
                      })
                    }
                  >
                    重启内核并重绑端口
                  </button>
                </div>
              )}
              <div className="ip-chips">
                {(ipItems.length
                  ? ipItems
                  : [
                      { key: "local", label: "本机", hint: "局域网地址", value: "—" },
                      {
                        key: "direct",
                        label: "直连",
                        hint: status?.tunEnabled
                          ? "TUN 开启时探测也可能被接管，仅供参考"
                          : "不走系统代理的公网出口",
                        value: "—",
                      },
                      { key: "proxy", label: "代理", hint: "经当前节点的公网 IP", value: "—" },
                    ]
                ).map((item) => (
                  <button
                    className="ip-chip"
                    key={item.key}
                    type="button"
                    title={`${item.label}：${item.value}`}
                    onClick={() => {
                      setIpModalOpen(true);
                      if (!ipItems.length && !ipLoading) void refreshOutboundIp();
                    }}
                  >
                    <span className="k">{ipChipLabel(item.label)}</span>
                    <span className="v">{shortIp(item.value)}</span>
                  </button>
                ))}
              </div>
            </div>

            <button
              className="policy kernel-item"
              type="button"
              onClick={() => setTab("kernel")}
            >
              <div>
                <div className="title">内核设置</div>
                <div className="desc">
                  {kernelVersion} · 端口 {status?.mixedPort ?? "-"} ·{" "}
                  {formatMemory(status?.memoryBytes)}
                </div>
              </div>
              <span className="chip">设置</span>
            </button>
            </div>
          </div>
        )}

        {tab === "kernel" && (
          <div className="page kernel-page">
            <div className="page-head">
              <div className="section-head">
                <button className="ghost linkish" type="button" onClick={() => setTab("home")}>
                  返回
                </button>
                <h2 className="page-title">内核设置</h2>
                <button
                  className="icon-ghost"
                  disabled={busy}
                  title="重启内核"
                  type="button"
                  onClick={() =>
                    void run(async () => {
                      setStatus(await restartCore());
                      await refreshSettings();
                      await refreshProxies();
                    })
                  }
                >
                  <RefreshIcon />
                </button>
              </div>
            </div>
            <div className="page-body stack">
            {status?.running && status.mixedPortBound === false ? (
              <div className="ip-warn">
                <div>
                  当前混合端口 {status.mixedPort} 未绑定到本应用内核。请关闭占用该端口的其他代理，或改用下方空闲端口后「应用并重启内核」。
                </div>
              </div>
            ) : null}

            <div className="card soft kernel-card">
              <div className="kernel-row">
                <div className="label">内核版本</div>
                <div className="kernel-right muted">{kernelVersion}</div>
              </div>
            </div>

            <div className="card soft kernel-card">
              <div className="kernel-row">
                <div className="label">混合端口</div>
                <div className="kernel-right">
                  <input
                    className="kernel-input"
                    type="text"
                    inputMode="numeric"
                    value={kernelDraft.mixedPort}
                    onChange={(e) => {
                      const next = onPortTyping(e.target.value);
                      if (next === null) return;
                      setKernelDraft((d) => ({ ...d, mixedPort: next }));
                    }}
                  />
                  <button
                    className="icon-ghost"
                    type="button"
                    title="随机端口"
                    onClick={() =>
                      setKernelDraft((d) => ({
                        ...d,
                        mixedPort: String(randomPort()),
                      }))
                    }
                  >
                    ⟳
                  </button>
                </div>
              </div>

              <div className="kernel-row">
                <div className="label">Socks 端口</div>
                <div className="kernel-right">
                  <input
                    className="kernel-input"
                    type="text"
                    inputMode="numeric"
                    disabled={!kernelDraft.socksOn}
                    value={kernelDraft.socksPort}
                    onChange={(e) => {
                      const next = onPortTyping(e.target.value);
                      if (next === null) return;
                      setKernelDraft((d) => ({ ...d, socksPort: next }));
                    }}
                  />
                  <button
                    className="icon-ghost"
                    type="button"
                    title="随机端口"
                    disabled={!kernelDraft.socksOn}
                    onClick={() =>
                      setKernelDraft((d) => ({
                        ...d,
                        socksPort: String(randomPort()),
                      }))
                    }
                  >
                    ⟳
                  </button>
                  <button
                    className={`switch ${kernelDraft.socksOn ? "on" : ""}`}
                    type="button"
                    onClick={() =>
                      setKernelDraft((d) => ({ ...d, socksOn: !d.socksOn }))
                    }
                  />
                </div>
              </div>

              <div className="kernel-row">
                <div className="label">Http 端口</div>
                <div className="kernel-right">
                  <input
                    className="kernel-input"
                    type="text"
                    inputMode="numeric"
                    disabled={!kernelDraft.httpOn}
                    value={kernelDraft.httpPort}
                    onChange={(e) => {
                      const next = onPortTyping(e.target.value);
                      if (next === null) return;
                      setKernelDraft((d) => ({ ...d, httpPort: next }));
                    }}
                  />
                  <button
                    className="icon-ghost"
                    type="button"
                    title="随机端口"
                    disabled={!kernelDraft.httpOn}
                    onClick={() =>
                      setKernelDraft((d) => ({
                        ...d,
                        httpPort: String(randomPort()),
                      }))
                    }
                  >
                    ⟳
                  </button>
                  <button
                    className={`switch ${kernelDraft.httpOn ? "on" : ""}`}
                    type="button"
                    onClick={() =>
                      setKernelDraft((d) => ({ ...d, httpOn: !d.httpOn }))
                    }
                  />
                </div>
              </div>

              <div className="kernel-row">
                <div className="label">控制端口</div>
                <div className="kernel-right">
                  <input
                    className="kernel-input"
                    type="text"
                    inputMode="numeric"
                    value={kernelDraft.controllerPort}
                    onChange={(e) => {
                      const next = onPortTyping(e.target.value);
                      if (next === null) return;
                      setKernelDraft((d) => ({ ...d, controllerPort: next }));
                    }}
                  />
                  <button
                    className="icon-ghost"
                    type="button"
                    title="随机端口"
                    onClick={() =>
                      setKernelDraft((d) => ({
                        ...d,
                        controllerPort: String(randomPort()),
                      }))
                    }
                  >
                    ⟳
                  </button>
                </div>
              </div>

              <div className="kernel-row">
                <div className="label">监听地址</div>
                <div className="kernel-right grow">
                  <input
                    className="kernel-input wide"
                    type="text"
                    placeholder="默认 · 局域网开启时为 *"
                    value={kernelDraft.bindAddress}
                    onChange={(e) =>
                      setKernelDraft((d) => ({
                        ...d,
                        bindAddress: e.target.value,
                      }))
                    }
                  />
                </div>
              </div>

              <div className="kernel-row">
                <div className="label">访问密钥</div>
                <div className="kernel-right grow">
                  <input
                    className="kernel-input wide"
                    type={secretVisible ? "text" : "password"}
                    value={kernelDraft.secret}
                    onChange={(e) =>
                      setKernelDraft((d) => ({ ...d, secret: e.target.value }))
                    }
                  />
                  <button
                    className="icon-ghost"
                    type="button"
                    title="重新生成"
                    onClick={() =>
                      setKernelDraft((d) => ({ ...d, secret: newSecret() }))
                    }
                  >
                    ⟳
                  </button>
                  <button
                    className="ghost linkish"
                    type="button"
                    title={secretVisible ? "隐藏" : "显示"}
                    onClick={() => setSecretVisible((v) => !v)}
                  >
                    {secretVisible ? "隐藏" : "显示"}
                  </button>
                </div>
              </div>

              <div className="kernel-row">
                <div className="label">IPv6</div>
                <button
                  className={`switch ${kernelDraft.ipv6 ? "on" : ""}`}
                  type="button"
                  onClick={() =>
                    setKernelDraft((d) => ({ ...d, ipv6: !d.ipv6 }))
                  }
                />
              </div>

              <div className="kernel-row">
                <div className="label">允许局域网连接</div>
                <button
                  className={`switch ${kernelDraft.allowLan ? "on" : ""}`}
                  type="button"
                  onClick={() =>
                    setKernelDraft((d) => ({ ...d, allowLan: !d.allowLan }))
                  }
                />
              </div>
            </div>

            <button
              className="primary"
              disabled={busy || !settingsPayload}
              type="button"
              onClick={() =>
                void run(async () => {
                  if (!settingsPayload) return;
                  const mixed = parsePortInput(kernelDraft.mixedPort, "混合端口");
                  const api = parsePortInput(kernelDraft.controllerPort, "控制端口");
                  const socks = kernelDraft.socksOn
                    ? parsePortInput(kernelDraft.socksPort, "Socks 端口")
                    : 0;
                  const http = kernelDraft.httpOn
                    ? parsePortInput(kernelDraft.httpPort, "Http 端口")
                    : 0;
                  const usedPorts = [mixed, api];
                  if (socks > 0) usedPorts.push(socks);
                  if (http > 0) usedPorts.push(http);
                  if (new Set(usedPorts).size !== usedPorts.length) {
                    throw new Error("混合 / Socks / Http / 控制端口不能重复");
                  }
                  const patch: AppSettings = {
                    ...settingsPayload.settings,
                    mixedPort: mixed,
                    socksPort: socks,
                    httpPort: http,
                    externalController: `127.0.0.1:${api}`,
                    secret: kernelDraft.secret.trim() || settingsPayload.settings.secret,
                    allowLan: kernelDraft.allowLan,
                    ipv6: kernelDraft.ipv6,
                    bindAddress: kernelDraft.bindAddress.trim(),
                  };
                  await applySettingsPayload(await saveSettings(patch));
                  setStatus(await getStatus());
                })
              }
            >
              应用并重启内核
            </button>
            </div>
          </div>
        )}

        {tab === "rules" && (() => {
          const q = ruleFilter.trim().toLowerCase();
          const match = (item: RuleListItem) => {
            if (!q) return true;
            return (
              item.payload.toLowerCase().includes(q) ||
              item.proxy.toLowerCase().includes(q) ||
              item.ruleType.toLowerCase().includes(q)
            );
          };
          const customRules = ruleItems.filter((i) => i.group === "custom" || i.custom).filter(match);
          const mainRules = ruleItems.filter((i) => i.group === "main" && !i.custom).filter(match);
          const internalRules = ruleItems
            .filter((i) => i.group === "internal" && !i.custom)
            .filter(match);
          const renderRule = (item: RuleListItem, idx: number) => (
            <div
              className={`rule-card ${item.custom ? "custom" : ""} ${item.enabled ? "" : "off"}`}
              key={item.id || `${item.ruleType}-${item.payload}-${item.proxy}-${idx}`}
            >
              <div className="rule-main">
                <div className="rule-payload" title={item.payload}>
                  {item.payload || "(空)"}
                </div>
                <div className="rule-tags">
                  <span className="chip">{item.ruleType}</span>
                  <span className="chip ok">{item.proxy || "-"}</span>
                  {item.custom ? <span className="chip">自定义</span> : null}
                </div>
              </div>
              <div className="rule-side">
                <div className="rule-meta">
                  {item.custom
                    ? item.enabled
                      ? "自定义"
                      : "已停用"
                    : item.size != null && item.size >= 0
                      ? `条目 ${item.size}`
                      : item.group === "internal"
                        ? "内部"
                        : "系统"}
                </div>
                {item.custom && item.id ? (
                  <div className="rule-actions">
                    <button
                      className={`switch ${item.enabled ? "on" : ""}`}
                      type="button"
                      disabled={busy}
                      onClick={() =>
                        void run(async () => {
                          const res = await setCustomRuleEnabled(item.id!, !item.enabled);
                          setRuleItems(res.items || []);
                          if (res.targets?.length) setRuleTargets(res.targets);
                        })
                      }
                    />
                    <button
                      className="ghost linkish"
                      type="button"
                      disabled={busy}
                      onClick={() =>
                        void run(async () => {
                          const res = await removeCustomRule(item.id!);
                          setRuleItems(res.items || []);
                          if (res.targets?.length) setRuleTargets(res.targets);
                        })
                      }
                    >
                      删除
                    </button>
                  </div>
                ) : null}
              </div>
            </div>
          );
          return (
            <div className="page page-rules">
              <div className="page-head">
                <h2 className="page-title">分流规则</h2>
                <div className="toolbar">
                  <input
                    value={ruleFilter}
                    onChange={(e) => setRuleFilter(e.target.value)}
                    placeholder="筛选过滤"
                  />
                  <button
                    className="primary"
                    type="button"
                    disabled={busy}
                    onClick={() => {
                      setRuleDraft({
                        ruleType: "DOMAIN-SUFFIX",
                        payload: "",
                        target: ruleTargets.includes("PROXY")
                          ? "PROXY"
                          : ruleTargets[0] || "DIRECT",
                      });
                      setRuleModalOpen(true);
                    }}
                  >
                    新增规则
                  </button>
                </div>
              </div>
              <div className="page-body stack">
              <p className="muted rules-hint">
                公司 / 国内 / 国外 / Final 可切换出站；自定义规则优先于系统分流。
                Telegram / Discord 已强制走「国外」；其它 IP 直连软件若 Final 为直连
                可能没网，可改 Final 为 PROXY。
              </p>

              <div className="card soft policy-card">
                {(
                  [
                    ...(policyGroups?.company
                      ? [
                          {
                            key: "company" as const,
                            label: "公司",
                            hint: "公司域名 / 网段",
                            group: policyGroups.company,
                            fallback: policyGroups.company.now || "DIRECT",
                          },
                        ]
                      : []),
                    {
                      key: "domestic" as const,
                      label: "国内",
                      hint: "GeoSite/GeoIP 中国流量",
                      group: policyGroups?.domestic,
                      fallback: "DIRECT",
                    },
                    {
                      key: "foreign" as const,
                      label: "国外",
                      hint: "geolocation-!cn",
                      group: policyGroups?.foreign,
                      fallback: "PROXY",
                    },
                    {
                      key: "final" as const,
                      label: "Final",
                      hint: "其余未命中流量",
                      group: policyGroups?.finalGroup,
                      fallback: "PROXY",
                    },
                  ]
                ).map((row) => {
                  const now = row.group?.now || row.fallback;
                  const options = row.group?.all?.length
                    ? row.group.all
                    : ["DIRECT", "PROXY"];
                  return (
                    <div className="policy-row" key={row.key}>
                      <div>
                        <div className="label">{row.label}</div>
                        <div className="desc">{row.hint}</div>
                      </div>
                      <select
                        className="policy-select"
                        disabled={busy || !status?.running}
                        value={now}
                        onChange={(e) =>
                          void run(async () => {
                            const name =
                              row.key === "company"
                                ? "公司"
                                : row.key === "domestic"
                                  ? "国内"
                                  : row.key === "foreign"
                                    ? "国外"
                                    : "Final";
                            const res = await setPolicyGroup(name, e.target.value);
                            setPolicyGroups(res);
                            await refreshRules();
                          })
                        }
                      >
                        {options.map((opt) => (
                          <option key={opt} value={opt}>
                            {opt === "DIRECT"
                              ? "直连 DIRECT"
                              : opt === "PROXY"
                                ? "代理 PROXY"
                                : opt}
                          </option>
                        ))}
                      </select>
                    </div>
                  );
                })}
              </div>

              <div className="rule-section">
                <div className="rule-section-head">
                  <span>自定义规则</span>
                  <span className="count">{customRules.length}</span>
                </div>
                <div className="rule-list">
                  {customRules.map(renderRule)}
                  {!customRules.length ? (
                    <p className="muted">还没有自定义规则，点上方新增。</p>
                  ) : null}
                </div>
              </div>

              <div className="rule-section">
                <div className="rule-section-head">
                  <span>系统分流</span>
                  <span className="count">{mainRules.length}</span>
                </div>
                <div className="rule-list">
                  {mainRules.map(renderRule)}
                  {!mainRules.length ? <p className="muted">暂无系统规则。</p> : null}
                </div>
              </div>

              {internalRules.length > 0 ? (
                <div className="rule-section">
                  <button
                    className="rule-section-toggle"
                    type="button"
                    onClick={() => setShowInternalRules((v) => !v)}
                  >
                    <span>内部规则（内网 / 探测）</span>
                    <span className="count">
                      {internalRules.length} · {showInternalRules ? "收起" : "展开"}
                    </span>
                  </button>
                  {showInternalRules ? (
                    <div className="rule-list">{internalRules.map(renderRule)}</div>
                  ) : null}
                </div>
              ) : null}
              </div>
            </div>
          );
        })()}

        {tab === "nodes" && (() => {
          const grouped = new Set(Object.values(airportNodes).flat());
          const orphans = (proxyGroup?.all || []).filter(
            (n) => n !== "DIRECT" && !grouped.has(n),
          );
          const renderNode = (name: string) => {
            const active = (proxyGroup?.now || status?.selectedProxy) === name;
            const delay = delays[name];
            return (
              <button
                key={name}
                className={`policy node-row ${active ? "active" : ""}`}
                disabled={busy}
                type="button"
                onClick={() =>
                  void run(async () => {
                    await selectProxyNode(name);
                    await refreshProxies();
                    await refreshStatus();
                    await refreshOutboundIp();
                  })
                }
              >
                <div>
                  <div className="title">
                    <NodeFlag name={name} />
                    <span>{name}</span>
                  </div>
                  <div className="desc">{active ? "当前出站" : "点击选用"}</div>
                </div>
                <span
                  className={`chip ${
                    delay === "bad" ? "bad" : typeof delay === "number" ? "ok" : ""
                  }`}
                  onClick={(e) => {
                    e.stopPropagation();
                    void run(async () => {
                      try {
                        const ms = await testProxyDelay(name);
                        setDelays((prev) => ({ ...prev, [name]: ms }));
                      } catch {
                        setDelays((prev) => ({ ...prev, [name]: "bad" }));
                      }
                    });
                  }}
                >
                  {delay === "bad"
                    ? "超时"
                    : typeof delay === "number"
                      ? `${delay} ms`
                      : "测速"}
                </span>
              </button>
            );
          };
          return (
            <div className="page page-proxies">
              <div className="page-head">
                <h2 className="page-title">代理</h2>
                <div className="toolbar">
                  <p className="muted" style={{ margin: 0, flex: 1 }}>
                    展开订阅选择节点，右侧可更新或管理
                  </p>
                  <button
                    className="secondary"
                    disabled={busy || !(proxyGroup?.all || []).length}
                    type="button"
                    onClick={() =>
                      void run(async () => {
                        const names = (proxyGroup?.all || []).filter((n) => n !== "DIRECT");
                        const results = await testProxyDelays(names);
                        const next: Record<string, number | "bad"> = { ...delays };
                        for (const item of results) {
                          next[item.name] = item.delay ?? "bad";
                        }
                        setDelays(next);
                      })
                    }
                  >
                    测速
                  </button>
                  <button
                    className="primary"
                    disabled={busy}
                    type="button"
                    onClick={() => {
                      setAirportUrl("");
                      setImportModalOpen(true);
                    }}
                  >
                    添加
                  </button>
                </div>
              </div>
              <div className="page-body stack">
                <section className="proxy-section company-item">
                  <div className="proxy-section-head">
                    <div className="proxy-section-main">
                      <div className="proxy-section-title">
                        <strong>公司 VPN</strong>
                      </div>
                      <div className="proxy-section-meta">
                        <span>
                          {settingsPayload?.company
                            ? settingsPayload.company.endpoint
                            : "内置分流模板，选择 conf 文件导入"}
                        </span>
                      </div>
                    </div>
                    <span
                      className={`chip ${settingsPayload?.company?.enabled ? "ok" : ""}`}
                    >
                      {settingsPayload?.company?.enabled
                        ? "已启用"
                        : settingsPayload?.company
                          ? "已关闭"
                          : "未导入"}
                    </span>
                    <button
                      className="secondary"
                      type="button"
                      onClick={() => setCompanyModalOpen(true)}
                    >
                      配置
                    </button>
                  </div>
                </section>

                {airports.map((item) => {
                  const used = (item.upload || 0) + (item.download || 0);
                  const total = item.total || 0;
                  const percent = total > 0 ? Math.min(100, (used / total) * 100) : 0;
                  const nodes = airportNodes[item.id] || [];
                  const expanded = !!proxyExpanded[item.id];
                  return (
                    <section className="proxy-section" key={item.id}>
                      <div className="proxy-section-head">
                        <button
                          className="proxy-section-toggle"
                          type="button"
                          onClick={() =>
                            setProxyExpanded((prev) => ({
                              ...prev,
                              [item.id]: !prev[item.id],
                            }))
                          }
                        >
                          <span className={`chevron ${expanded ? "open" : ""}`}>›</span>
                          <div className="proxy-section-main">
                            <div className="proxy-section-title">
                              <strong>{item.name}</strong>
                              <span className="count">{nodes.length || item.nodeCount}</span>
                            </div>
                            <div className="proxy-section-meta">
                              <span>
                                {total > 0 ? `${formatBytes(used)} / ${formatBytes(total)}` : ""}
                              </span>
                              <span>{formatExpire(item.expire)}</span>
                            </div>
                            {total > 0 ? (
                              <div className="proxy-group-bar">
                                <span style={{ width: `${percent}%` }} />
                              </div>
                            ) : null}
                          </div>
                        </button>
                        <div className="proxy-section-actions">
                          <button
                            className="icon-ghost"
                            type="button"
                            title="更新"
                            disabled={busy || !item.url}
                            onClick={() =>
                              void run(async () => {
                                await applySettingsPayload(await updateAirport(item.id));
                              })
                            }
                          >
                            <RefreshIcon />
                          </button>
                          <button
                            className="icon-ghost"
                            type="button"
                            title="管理"
                            disabled={busy}
                            onClick={() =>
                              setManageAirport({
                                id: item.id,
                                name: item.name,
                                url: item.url || "",
                              })
                            }
                          >
                            ⓘ
                          </button>
                        </div>
                      </div>
                      {expanded ? (
                        <div className="proxy-section-body">
                          <div className="node-list">
                            {nodes.map((name) => renderNode(name))}
                            {!nodes.length ? (
                              <p className="muted">该订阅暂无节点，可点更新拉取。</p>
                            ) : null}
                          </div>
                        </div>
                      ) : null}
                    </section>
                  );
                })}

                {!airports.length ? (
                  <p className="muted">还没有订阅，点上方「添加」导入。</p>
                ) : null}

                {orphans.length ? (
                  <section className="proxy-section">
                    <div className="proxy-section-head">
                      <div className="proxy-section-main">
                        <div className="proxy-section-title">
                          <strong>其他节点</strong>
                          <span className="count">{orphans.length}</span>
                        </div>
                      </div>
                    </div>
                    <div className="proxy-section-body">
                      <div className="node-list">
                        {orphans.map((name) => renderNode(name))}
                      </div>
                    </div>
                  </section>
                ) : null}
              </div>
            </div>
          );
        })()}

        {tab === "logs" && (
          <div className="page page-logs">
            <div className="page-head">
              <h2 className="page-title">实时日志</h2>
              <div className="toolbar">
                <input
                  value={logFilter}
                  onChange={(e) => setLogFilter(e.target.value)}
                  placeholder="筛选过滤"
                />
                <select value={logLevel} onChange={(e) => setLogLevel(e.target.value)}>
                  <option value="ALL">全部</option>
                  <option value="INFO">INFO</option>
                  <option value="WARN">WARN</option>
                  <option value="ERROR">ERROR</option>
                  <option value="DEBUG">DEBUG</option>
                </select>
                <button
                  className="icon-ghost log-tool danger-icon"
                  disabled={busy}
                  title="清空"
                  onClick={() =>
                    void run(async () => {
                      await clearLogs();
                      setLogEntries([]);
                    })
                  }
                >
                  <TrashIcon />
                </button>
              </div>
            </div>
            <div className="page-body log-list">
              {filteredLogs.map((item) => (
                <div className="log-card" key={item.id}>
                  <div className="head">
                    <span className={`level lv-${item.level.toLowerCase()}`}>{item.level}</span>
                    <span className="time">{item.time}</span>
                  </div>
                  <div className="body">{item.body}</div>
                </div>
              ))}
              {!filteredLogs.length ? <p className="muted">暂无日志。</p> : null}
            </div>
          </div>
        )}

        {tab === "settings" && (
          <div className="page page-settings">
            <div className="page-head">
              <h2 className="page-title">设置</h2>
            </div>
            <div className="page-body stack">
              <div className="setting-row">
                <div>
                  <div className="label">开机自启</div>
                  <div className="desc">
                    登录后以管理员权限启动（需当前已是管理员；用于 TUN）
                  </div>
                </div>
                <button
                  className={`switch ${autostart ? "on" : ""}`}
                  disabled={busy}
                  onClick={() =>
                    void run(async () => {
                      setAutostartState(await setAutostart(!autostart));
                    })
                  }
                />
              </div>
            </div>
            <button
              className="danger quit-btn"
              disabled={busy}
              onClick={() => void quitApp()}
            >
              退出应用
            </button>
          </div>
        )}
      </main>

      {manageAirport ? (
        <div className="modal-mask" onClick={() => setManageAirport(null)}>
          <div className="modal modal-sm" onClick={(e) => e.stopPropagation()}>
            <div className="modal-head">
              <h3>{manageAirport.name}</h3>
              <button className="ghost" type="button" onClick={() => setManageAirport(null)}>
                关闭
              </button>
            </div>
            <p className="hint">更新、测速、重命名与删除在此集中处理。</p>
            <div className="manage-actions">
              <button
                className="secondary"
                type="button"
                disabled={busy || !manageAirport.url}
                onClick={() =>
                  void run(async () => {
                    await applySettingsPayload(await updateAirport(manageAirport.id));
                    setManageAirport(null);
                  })
                }
              >
                更新订阅
              </button>
              <button
                className="secondary"
                type="button"
                disabled={busy}
                onClick={() =>
                  void run(async () => {
                    const nodes = airportNodes[manageAirport.id] || [];
                    if (nodes.length) {
                      const results = await testProxyDelays(nodes);
                      const next: Record<string, number | "bad"> = { ...delays };
                      for (const r of results) next[r.name] = r.delay ?? "bad";
                      setDelays(next);
                    }
                    setManageAirport(null);
                  })
                }
              >
                测速此订阅
              </button>
              <button
                className="secondary"
                type="button"
                disabled={busy}
                onClick={() => {
                  setRenameModal({ id: manageAirport.id, name: manageAirport.name });
                  setRenameValue(manageAirport.name);
                  setManageAirport(null);
                }}
              >
                重命名
              </button>
              <button
                className="danger"
                type="button"
                disabled={busy}
                onClick={() =>
                  void run(async () => {
                    await applySettingsPayload(await removeAirport(manageAirport.id));
                    setManageAirport(null);
                  })
                }
              >
                删除
              </button>
            </div>
          </div>
        </div>
      ) : null}

      {importModalOpen ? (
        <div className="modal-mask" onClick={() => setImportModalOpen(false)}>
          <div className="modal modal-sm" onClick={(e) => e.stopPropagation()}>
            <div className="modal-head">
              <h3>添加订阅</h3>
              <button className="ghost" type="button" onClick={() => setImportModalOpen(false)}>
                关闭
              </button>
            </div>
            <p className="hint">粘贴订阅链接，或选择本地 Clash / Mihomo YAML 文件。</p>
            <div className="field">
              <label>订阅网址</label>
              <input
                value={airportUrl}
                onChange={(e) => setAirportUrl(e.target.value)}
                placeholder="https://..."
              />
            </div>
            <div className="row" style={{ marginTop: 12 }}>
              <button
                className="secondary"
                type="button"
                disabled={busy}
                onClick={() =>
                  void run(async () => {
                    const selectedFile = await open({
                      multiple: false,
                      filters: [{ name: "YAML", extensions: ["yaml", "yml", "txt"] }],
                    });
                    if (typeof selectedFile === "string") {
                      await applySettingsPayload(await addAirportFile(selectedFile));
                      setImportModalOpen(false);
                      setAirportUrl("");
                    }
                  })
                }
              >
                本地文件
              </button>
              <button
                className="primary"
                type="button"
                disabled={busy || !airportUrl.trim()}
                onClick={() =>
                  void run(async () => {
                    await applySettingsPayload(await addAirportUrl(airportUrl.trim()));
                    setImportModalOpen(false);
                    setAirportUrl("");
                  })
                }
              >
                导入
              </button>
            </div>
          </div>
        </div>
      ) : null}

      {ipModalOpen ? (
        <div className="modal-mask" onClick={() => setIpModalOpen(false)}>
          <div className="modal modal-sm" onClick={(e) => e.stopPropagation()}>
            <div className="modal-head">
              <h3>当前 IP</h3>
              <button className="ghost" type="button" onClick={() => setIpModalOpen(false)}>
                关闭
              </button>
            </div>
            <p className="hint">本机 / 直连 / 代理出口详情。TUN 开启时直连探测仅供参考。</p>
            <div className="ip-detail-list">
              {(ipItems.length
                ? ipItems
                : [
                    { key: "local", label: "本机 IP", hint: "局域网地址", value: "—" },
                    {
                      key: "direct",
                      label: "直连 IP",
                      hint: status?.tunEnabled
                        ? "TUN 开启时探测也可能被接管，仅供参考"
                        : "不走系统代理的公网出口",
                      value: "—",
                    },
                    { key: "proxy", label: "代理出口", hint: "经当前节点的公网 IP", value: "—" },
                  ]
              ).map((item) => (
                <div className="ip-detail-row" key={item.key}>
                  <div className="ip-meta">
                    <span className="k">{item.label}</span>
                    <span className="hint">{item.hint}</span>
                  </div>
                  <span className="v">{item.value}</span>
                </div>
              ))}
            </div>
            <div className="row" style={{ marginTop: 12 }}>
              <button
                className="primary"
                type="button"
                disabled={ipLoading}
                onClick={() => void refreshOutboundIp()}
              >
                {ipLoading ? "查询中…" : "刷新"}
              </button>
            </div>
          </div>
        </div>
      ) : null}

      <nav className="tabs">
        {(
          [
            ["home", "首页"],
            ["nodes", "代理"],
            ["rules", "规则"],
            ["logs", "日志"],
            ["settings", "设置"],
          ] as const
        ).map(([id, label]) => (
          <button
            key={id}
            className={`tab ${tab === id ? "active" : ""}`}
            onClick={() => setTab(id)}
          >
            {label}
          </button>
        ))}
      </nav>

      {ruleModalOpen ? (
        <div className="modal-mask" onClick={() => setRuleModalOpen(false)}>
          <div className="modal modal-sm" onClick={(e) => e.stopPropagation()}>
            <div className="modal-head">
              <h3>新增规则</h3>
              <button className="ghost" type="button" onClick={() => setRuleModalOpen(false)}>
                关闭
              </button>
            </div>
            <p className="hint">匹配到的地址将按你选择的出站转发，优先级高于内置分流。</p>
            <div className="field">
              <label>类型</label>
              <select
                value={ruleDraft.ruleType}
                onChange={(e) =>
                  setRuleDraft((d) => ({ ...d, ruleType: e.target.value }))
                }
              >
                <option value="DOMAIN">DOMAIN（完整域名）</option>
                <option value="DOMAIN-SUFFIX">DOMAIN-SUFFIX（后缀）</option>
                <option value="DOMAIN-KEYWORD">DOMAIN-KEYWORD（关键字）</option>
                <option value="IP-CIDR">IP-CIDR</option>
                <option value="IP-CIDR6">IP-CIDR6</option>
              </select>
            </div>
            <div className="field">
              <label>匹配内容</label>
              <input
                autoFocus
                value={ruleDraft.payload}
                onChange={(e) =>
                  setRuleDraft((d) => ({ ...d, payload: e.target.value }))
                }
                placeholder="例如 google.com 或 10.0.0.0/24"
              />
            </div>
            <div className="field">
              <label>出站</label>
              <select
                value={ruleDraft.target}
                onChange={(e) =>
                  setRuleDraft((d) => ({ ...d, target: e.target.value }))
                }
              >
                {ruleTargets.map((t) => (
                  <option key={t} value={t}>
                    {t}
                  </option>
                ))}
              </select>
            </div>
            <div className="row">
              <button
                className="secondary"
                type="button"
                onClick={() => setRuleModalOpen(false)}
              >
                取消
              </button>
              <button
                className="primary"
                type="button"
                disabled={busy || !ruleDraft.payload.trim()}
                onClick={() =>
                  void run(async () => {
                    const res = await addCustomRule({
                      ruleType: ruleDraft.ruleType,
                      payload: ruleDraft.payload.trim(),
                      target: ruleDraft.target,
                      enabled: true,
                    });
                    setRuleItems(res.items || []);
                    if (res.targets?.length) setRuleTargets(res.targets);
                    setRuleModalOpen(false);
                  })
                }
              >
                添加
              </button>
            </div>
          </div>
        </div>
      ) : null}

      {renameModal ? (
        <div
          className="modal-mask"
          onClick={() => {
            setRenameModal(null);
            setRenameValue("");
          }}
        >
          <div className="modal modal-sm" onClick={(e) => e.stopPropagation()}>
            <div className="modal-head">
              <h3>重命名订阅</h3>
              <button
                className="ghost"
                type="button"
                onClick={() => {
                  setRenameModal(null);
                  setRenameValue("");
                }}
              >
                关闭
              </button>
            </div>
            <div className="field">
              <label>名称</label>
              <input
                autoFocus
                value={renameValue}
                onChange={(e) => setRenameValue(e.target.value)}
                onKeyDown={(e) => {
                  if (e.key === "Enter") {
                    e.preventDefault();
                    const next = renameValue.trim();
                    if (!next || busy) return;
                    void run(async () => {
                      await applySettingsPayload(
                        await renameAirport(renameModal.id, next),
                      );
                      setRenameModal(null);
                      setRenameValue("");
                    });
                  }
                }}
                placeholder="订阅显示名称"
              />
            </div>
            <div className="row">
              <button
                className="secondary"
                type="button"
                onClick={() => {
                  setRenameModal(null);
                  setRenameValue("");
                }}
              >
                取消
              </button>
              <button
                className="primary"
                type="button"
                disabled={busy || !renameValue.trim()}
                onClick={() =>
                  void run(async () => {
                    const next = renameValue.trim();
                    if (!next) return;
                    await applySettingsPayload(
                      await renameAirport(renameModal.id, next),
                    );
                    setRenameModal(null);
                    setRenameValue("");
                  })
                }
              >
                保存
              </button>
            </div>
          </div>
        </div>
      ) : null}

      {companyModalOpen ? (
        <div className="modal-mask" onClick={() => setCompanyModalOpen(false)}>
          <div className="modal modal-sm" onClick={(e) => e.stopPropagation()}>
            <div className="modal-head">
              <h3>公司 VPN</h3>
              <button className="ghost" onClick={() => setCompanyModalOpen(false)}>
                关闭
              </button>
            </div>
            <p className="hint">
              域名 / 网段 / 冲突 IP 已内置；conf 中的密钥、Endpoint、Address、DNS
              会覆盖模板。选择文件导入后用开关启停。
            </p>
            <div className="company-enable-row">
              <div>
                <div className="label">启用公司 VPN</div>
                <div className="desc">
                  {settingsPayload?.company
                    ? "关闭后保留已导入配置，可随时再开"
                    : "请先选择 conf 文件导入"}
                </div>
              </div>
              <button
                className={`switch ${settingsPayload?.company?.enabled ? "on" : ""}`}
                type="button"
                disabled={busy || !settingsPayload?.company}
                onClick={() =>
                  void run(async () => {
                    const next = !settingsPayload?.company?.enabled;
                    await applySettingsPayload(await setCompanyEnabled(next));
                    setCompanyCheck("idle");
                    setCompanyCheckText(null);
                  })
                }
              />
            </div>
            <div className="row company-actions">
              <button
                className="primary"
                disabled={busy}
                onClick={() =>
                  void run(async () => {
                    const selected = await open({
                      multiple: false,
                      filters: [{ name: "WireGuard", extensions: ["conf", "txt"] }],
                    });
                    if (typeof selected !== "string") return;
                    const confText = await readTextFile(selected);
                    await applySettingsPayload(await enableCompanyFromConf(confText));
                    setCompanyCheck("idle");
                    setCompanyCheckText(null);
                  })
                }
              >
                {settingsPayload?.company ? "重新导入" : "选择文件导入"}
              </button>
              {settingsPayload?.company ? (
                <button
                  className="ghost linkish"
                  disabled={busy}
                  onClick={() =>
                    void run(async () => {
                      await applySettingsPayload(await clearCompanyWg());
                      setCompanyCheck("idle");
                      setCompanyCheckText(null);
                    })
                  }
                >
                  清除配置
                </button>
              ) : null}
            </div>
            {settingsPayload?.company ? (
              <div className="company-status">
                <div>
                  {settingsPayload.company.enabled ? "已启用" : "已关闭"} ·{" "}
                  {settingsPayload.company.name || "公司VPN"}
                </div>
                <div>端点：{settingsPayload.company.endpoint}</div>
                <div>
                  域名：
                  {settingsPayload.company.domain ||
                    settingsPayload.company.domains.join(", ")}
                </div>
                <div>网段：{settingsPayload.company.cidrs.join(", ")}</div>
                <div>DNS：{settingsPayload.company.dnsServers.join(", ")}</div>
                {settingsPayload.company.allowedIps?.length ? (
                  <div>
                    Allowed IPs：{settingsPayload.company.allowedIps.join(", ")}
                  </div>
                ) : null}
                <div>
                  MTU：{settingsPayload.company.mtu ?? "自动"} · Keepalive：
                  {settingsPayload.company.persistentKeepalive ?? 0}s
                </div>
                {settingsPayload.company.conflictIps?.length ? (
                  <div>冲突 IP：{settingsPayload.company.conflictIps.join(", ")}</div>
                ) : null}
              </div>
            ) : null}
          </div>
        </div>
      ) : null}
    </div>
  );
}

export default App;
