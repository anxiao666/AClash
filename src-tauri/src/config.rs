use serde_yaml::{Mapping, Value};

use crate::airport;
use crate::company::{self, CompanyProfile};
use crate::error::{AppError, AppResult};
use crate::paths;
use crate::settings::AppSettings;

pub fn regenerate(settings: &AppSettings) -> AppResult<()> {
    let proxies = airport::collect_enabled_proxies(settings)?;
    let company = company::load_enabled()?;
    let yaml = build_config(settings, &proxies, company.as_ref())?;
    std::fs::write(paths::config_path()?, yaml)?;
    Ok(())
}

pub fn build_config(
    settings: &AppSettings,
    airport_proxies: &[Value],
    company: Option<&CompanyProfile>,
) -> AppResult<String> {
    let mut root = Mapping::new();

    root.insert(
        Value::from("mixed-port"),
        Value::from(settings.mixed_port as i64),
    );
    if settings.socks_port > 0 {
        root.insert(
            Value::from("socks-port"),
            Value::from(settings.socks_port as i64),
        );
    }
    if settings.http_port > 0 {
        root.insert(Value::from("port"), Value::from(settings.http_port as i64));
    }
    root.insert(Value::from("allow-lan"), Value::from(settings.allow_lan));
    let bind = settings.bind_address.trim();
    if !bind.is_empty() {
        root.insert(Value::from("bind-address"), Value::from(bind));
    } else if settings.allow_lan {
        root.insert(Value::from("bind-address"), Value::from("*"));
    }
    root.insert(Value::from("mode"), Value::from("rule"));
    root.insert(Value::from("log-level"), Value::from("info"));
    root.insert(Value::from("ipv6"), Value::from(settings.ipv6));
    // Match Clash Party: second HEAD RTT only, exclude dial/handshake overhead.
    root.insert(Value::from("unified-delay"), Value::from(true));
    root.insert(Value::from("tcp-concurrent"), Value::from(true));
    root.insert(
        Value::from("external-controller"),
        Value::from(settings.external_controller.clone()),
    );
    root.insert(Value::from("secret"), Value::from(settings.secret.clone()));
    root.insert(Value::from("find-process-mode"), Value::from("strict"));
    root.insert(Value::from("geodata-mode"), Value::from(true));
    root.insert(Value::from("geo-auto-update"), Value::from(true));
    root.insert(Value::from("geo-update-interval"), Value::from(24));

    let mut geox = Mapping::new();
    geox.insert(
        Value::from("geoip"),
        Value::from("https://cdn.jsdelivr.net/gh/MetaCubeX/meta-rules-dat@release/geoip.dat"),
    );
    geox.insert(
        Value::from("geosite"),
        Value::from("https://cdn.jsdelivr.net/gh/MetaCubeX/meta-rules-dat@release/geosite.dat"),
    );
    geox.insert(
        Value::from("mmdb"),
        Value::from("https://cdn.jsdelivr.net/gh/MetaCubeX/meta-rules-dat@release/country.mmdb"),
    );
    root.insert(Value::from("geox-url"), Value::Mapping(geox));

    // DNS
    let mut dns = Mapping::new();
    dns.insert(Value::from("enable"), Value::from(true));
    dns.insert(Value::from("ipv6"), Value::from(settings.ipv6));
    dns.insert(Value::from("enhanced-mode"), Value::from("fake-ip"));
    dns.insert(Value::from("fake-ip-range"), Value::from("198.18.0.1/16"));
    dns.insert(
        Value::from("default-nameserver"),
        Value::Sequence(vec![Value::from("223.5.5.5"), Value::from("119.29.29.29")]),
    );
    dns.insert(
        Value::from("nameserver"),
        Value::Sequence(vec![Value::from("223.5.5.5"), Value::from("119.29.29.29")]),
    );
    dns.insert(
        Value::from("proxy-server-nameserver"),
        Value::Sequence(vec![Value::from("223.5.5.5"), Value::from("119.29.29.29")]),
    );

    let mut fake_ip_filter = vec![
        Value::from("geosite:connectivity-check"),
        Value::from("*.lan"),
        Value::from("*.local"),
        Value::from("*.localdomain"),
        Value::from("+.pool.ntp.org"),
        Value::from("time.*.com"),
        Value::from("time.*.gov"),
        Value::from("ntp.*.com"),
        Value::from("*.msftconnecttest.com"),
        Value::from("*.msftncsi.com"),
    ];

    if let Some(c) = company {
        let dns_via = format!("{}#{}", c.dns, c.name);
        let mut policy = Mapping::new();
        let mut company_domains = vec![c.domain.clone()];
        company_domains.extend(
            c.custom_company
                .iter()
                .map(|d| d.trim().to_string())
                .filter(|d| !d.is_empty()),
        );
        for domain in company_domains {
            fake_ip_filter.insert(0, Value::from(format!("+.{domain}")));
            policy.insert(
                Value::from(format!("+.{domain}")),
                Value::from(dns_via.clone()),
            );
        }
        dns.insert(Value::from("nameserver-policy"), Value::Mapping(policy));
    }
    dns.insert(
        Value::from("fake-ip-filter"),
        Value::Sequence(fake_ip_filter),
    );
    root.insert(Value::from("dns"), Value::Mapping(dns));

    // TUN
    let mut tun = Mapping::new();
    tun.insert(Value::from("enable"), Value::from(settings.tun_enabled));
    tun.insert(Value::from("stack"), Value::from("mixed"));
    tun.insert(Value::from("auto-route"), Value::from(true));
    tun.insert(Value::from("auto-detect-interface"), Value::from(true));
    tun.insert(
        Value::from("dns-hijack"),
        Value::Sequence(vec![Value::from("any:53")]),
    );
    if let Some(c) = company {
        if !c.conflict_ips.is_empty() {
            let mut routes = vec![Value::from("0.0.0.0/1"), Value::from("128.0.0.0/1")];
            routes.extend(c.conflict_ips.iter().cloned().map(Value::from));
            tun.insert(Value::from("route-address"), Value::Sequence(routes));
        }
    }
    root.insert(Value::from("tun"), Value::Mapping(tun));

    // sniff
    let mut sniff = Mapping::new();
    sniff.insert(Value::from("enable"), Value::from(true));
    sniff.insert(Value::from("force-dns-mapping"), Value::from(true));
    sniff.insert(Value::from("override-destination"), Value::from(false));
    let mut sniff_protocols = Mapping::new();
    sniff_protocols.insert(
        Value::from("TLS"),
        Value::Mapping({
            let mut m = Mapping::new();
            m.insert(
                Value::from("ports"),
                Value::Sequence(vec![Value::from(443), Value::from(8443)]),
            );
            m
        }),
    );
    sniff_protocols.insert(
        Value::from("HTTP"),
        Value::Mapping({
            let mut m = Mapping::new();
            m.insert(
                Value::from("ports"),
                Value::Sequence(vec![Value::from(80), Value::from(8080), Value::from(8880)]),
            );
            m
        }),
    );
    sniff.insert(Value::from("sniff"), Value::Mapping(sniff_protocols));
    root.insert(Value::from("sniffer"), Value::Mapping(sniff));

    // proxies
    let mut proxies: Vec<Value> = airport_proxies
        .iter()
        .filter(|p| {
            let name = p.get("name").and_then(|v| v.as_str()).unwrap_or("");
            !is_junk_node(name)
        })
        .cloned()
        .collect();
    if let Some(c) = company {
        proxies.retain(|p| p.get("name").and_then(|v| v.as_str()) != Some(c.name.as_str()));
        proxies.push(wireguard_proxy_value(c));
    }
    root.insert(Value::from("proxies"), Value::Sequence(proxies.clone()));

    let names: Vec<String> = proxies
        .iter()
        .filter_map(|p| {
            p.get("name")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string())
        })
        .filter(|n| company.map(|c| n != &c.name).unwrap_or(true))
        .filter(|n| !is_junk_node(n))
        .collect();

    let selected = settings
        .selected_proxy
        .clone()
        .filter(|n| names.iter().any(|x| x == n))
        .or_else(|| names.first().cloned());

    let exclude = company
        .map(|c| company::exclude_node_regex(&c.name))
        .unwrap_or_else(|| company::exclude_node_regex("公司VPN"));

    let mut proxy_group = Mapping::new();
    proxy_group.insert(Value::from("name"), Value::from("PROXY"));
    proxy_group.insert(Value::from("type"), Value::from("select"));
    proxy_group.insert(Value::from("exclude-filter"), Value::from(exclude));
    let mut proxy_group_proxies: Vec<Value> = if names.is_empty() {
        vec![Value::from("DIRECT")]
    } else {
        names.iter().cloned().map(Value::from).collect()
    };
    if let Some(sel) = selected {
        if let Some(pos) = proxy_group_proxies
            .iter()
            .position(|v| v.as_str() == Some(sel.as_str()))
        {
            let item = proxy_group_proxies.remove(pos);
            proxy_group_proxies.insert(0, item);
        }
    }
    proxy_group.insert(Value::from("proxies"), Value::Sequence(proxy_group_proxies));

    // Party-like selectable policies: 公司 / 国内 / 国外 / Final
    let mut groups = vec![Value::Mapping(proxy_group)];
    if let Some(c) = company {
        let members = vec![c.name.clone(), "DIRECT".into(), "PROXY".into()];
        let selected = if settings.policy_company.trim().is_empty()
            || !members.iter().any(|m| m == &settings.policy_company)
        {
            c.name.as_str()
        } else {
            settings.policy_company.as_str()
        };
        groups.push(Value::Mapping(policy_select_group_owned(
            crate::settings::POLICY_COMPANY,
            selected,
            &members,
        )));
    }
    groups.push(Value::Mapping(policy_select_group(
        crate::settings::POLICY_DOMESTIC,
        &settings.policy_domestic,
        &["DIRECT", "PROXY"],
    )));
    groups.push(Value::Mapping(policy_select_group(
        crate::settings::POLICY_FOREIGN,
        &settings.policy_foreign,
        &["PROXY", "DIRECT"],
    )));
    groups.push(Value::Mapping(policy_select_group(
        crate::settings::POLICY_FINAL,
        &settings.policy_final,
        &["PROXY", "DIRECT"],
    )));
    root.insert(Value::from("proxy-groups"), Value::Sequence(groups));

    // rules — user custom first, then company, then domestic/foreign/final policies
    let mut rules: Vec<Value> = Vec::new();
    for rule in &settings.custom_rules {
        if let Some(line) = rule.to_clash_line() {
            rules.push(Value::from(line));
        }
    }
    if let Some(c) = company {
        let company_policy = crate::settings::POLICY_COMPANY;
        rules.push(Value::from(format!("DOMAIN,{},DIRECT", c.server)));
        rules.push(Value::from(format!(
            "DOMAIN-SUFFIX,{},{}",
            c.domain, company_policy
        )));
        for d in &c.custom_company {
            if !d.trim().is_empty() {
                rules.push(Value::from(format!(
                    "DOMAIN-SUFFIX,{},{}",
                    d.trim(),
                    company_policy
                )));
            }
        }
        for cidr in &c.cidrs {
            if cidr.contains(':') {
                rules.push(Value::from(format!(
                    "IP-CIDR6,{},{},no-resolve",
                    cidr, company_policy
                )));
            } else {
                rules.push(Value::from(format!(
                    "IP-CIDR,{},{},no-resolve",
                    cidr, company_policy
                )));
            }
        }
        for d in &c.custom_direct {
            if !d.trim().is_empty() {
                rules.push(Value::from(format!(
                    "DOMAIN-SUFFIX,{},{}",
                    d.trim(),
                    crate::settings::POLICY_DOMESTIC
                )));
            }
        }
        for d in &c.custom_proxy {
            if !d.trim().is_empty() {
                rules.push(Value::from(format!(
                    "DOMAIN-SUFFIX,{},{}",
                    d.trim(),
                    crate::settings::POLICY_FOREIGN
                )));
            }
        }
    }

    // Avoid GEOIP,private: it can swallow fake-ip (198.18.0.0/16) and force DIRECT.
    for cidr in [
        "0.0.0.0/8",
        "10.0.0.0/8",
        "127.0.0.0/8",
        "169.254.0.0/16",
        "172.16.0.0/12",
        "192.168.0.0/16",
        "224.0.0.0/4",
        "240.0.0.0/4",
    ] {
        rules.push(Value::from(format!("IP-CIDR,{cidr},DIRECT,no-resolve")));
    }
    // Force IP echo hosts through PROXY so home "代理出口" tracks the selected node.
    for host in [
        "ipify.org",
        "ip.sb",
        "ifconfig.me",
        "ipinfo.io",
        "icanhazip.com",
    ] {
        rules.push(Value::from(format!("DOMAIN-SUFFIX,{host},PROXY")));
    }
    rules.push(Value::from("AND,((DST-PORT,443),(NETWORK,UDP)),REJECT"));
    rules.push(Value::from("GEOSITE,private,DIRECT"));
    // IP-direct apps: keep explicit bypass so Final can stay DIRECT.
    rules.push(Value::from(format!(
        "GEOSITE,telegram,{}",
        crate::settings::POLICY_FOREIGN
    )));
    rules.push(Value::from(format!(
        "PROCESS-NAME,Telegram.exe,{}",
        crate::settings::POLICY_FOREIGN
    )));
    rules.push(Value::from(format!(
        "PROCESS-NAME,Telegram,{}",
        crate::settings::POLICY_FOREIGN
    )));
    rules.push(Value::from(format!(
        "GEOSITE,discord,{}",
        crate::settings::POLICY_FOREIGN
    )));
    rules.push(Value::from(format!(
        "PROCESS-NAME,Discord.exe,{}",
        crate::settings::POLICY_FOREIGN
    )));
    rules.push(Value::from(format!(
        "PROCESS-NAME,Discord,{}",
        crate::settings::POLICY_FOREIGN
    )));
    rules.push(Value::from(format!(
        "PROCESS-NAME,DiscordPTB.exe,{}",
        crate::settings::POLICY_FOREIGN
    )));
    rules.push(Value::from(format!(
        "PROCESS-NAME,DiscordCanary.exe,{}",
        crate::settings::POLICY_FOREIGN
    )));
    rules.push(Value::from(format!(
        "GEOSITE,cn,{}",
        crate::settings::POLICY_DOMESTIC
    )));
    rules.push(Value::from(format!(
        "GEOIP,CN,{}",
        crate::settings::POLICY_DOMESTIC
    )));
    rules.push(Value::from(format!(
        "GEOSITE,geolocation-!cn,{}",
        crate::settings::POLICY_FOREIGN
    )));
    rules.push(Value::from(format!(
        "MATCH,{}",
        crate::settings::POLICY_FINAL
    )));
    root.insert(Value::from("rules"), Value::Sequence(rules));

    let doc = Value::Mapping(root);
    let mut out = String::from("# Generated by AClash. Do not edit manually.\n");
    out.push_str(&serde_yaml::to_string(&doc).map_err(AppError::from)?);
    Ok(out)
}

fn is_junk_node(name: &str) -> bool {
    let keys = [
        "剩余", "到期", "过期", "重置", "流量", "官网", "套餐", "客服", "公告",
    ];
    keys.iter().any(|k| name.contains(k))
}

fn policy_select_group(name: &str, selected: &str, order: &[&str]) -> Mapping {
    let owned: Vec<String> = order.iter().map(|s| (*s).to_string()).collect();
    let want = AppSettings::normalize_policy_selection(selected, order[0]);
    policy_select_group_owned(name, &want, &owned)
}

fn policy_select_group_owned(name: &str, selected: &str, order: &[String]) -> Mapping {
    let mut group = Mapping::new();
    group.insert(Value::from("name"), Value::from(name));
    group.insert(Value::from("type"), Value::from("select"));
    let mut members: Vec<Value> = order.iter().cloned().map(Value::from).collect();
    let fallback = order.first().map(|s| s.as_str()).unwrap_or("DIRECT");
    let want = if order.iter().any(|m| m == selected) {
        selected
    } else {
        fallback
    };
    if let Some(pos) = members.iter().position(|v| v.as_str() == Some(want)) {
        let item = members.remove(pos);
        members.insert(0, item);
    }
    group.insert(Value::from("proxies"), Value::Sequence(members));
    group
}

fn wireguard_proxy_value(c: &CompanyProfile) -> Value {
    let mut map = Mapping::new();
    map.insert(Value::from("name"), Value::from(c.name.clone()));
    map.insert(Value::from("type"), Value::from("wireguard"));
    map.insert(Value::from("server"), Value::from(c.server.clone()));
    map.insert(Value::from("port"), Value::from(c.port as i64));
    map.insert(Value::from("ip"), Value::from(c.client_ip.clone()));
    map.insert(
        Value::from("private-key"),
        Value::from(c.private_key.clone()),
    );
    map.insert(Value::from("public-key"), Value::from(c.public_key.clone()));
    if let Some(psk) = &c.pre_shared_key {
        map.insert(Value::from("pre-shared-key"), Value::from(psk.clone()));
    }
    if let Some(mtu) = c.mtu {
        map.insert(Value::from("mtu"), Value::from(mtu as i64));
    }
    map.insert(Value::from("udp"), Value::from(true));
    if let Some(keepalive) = c.persistent_keepalive {
        map.insert(
            Value::from("persistent-keepalive"),
            Value::from(keepalive as i64),
        );
    }
    map.insert(Value::from("remote-dns-resolve"), Value::from(true));
    map.insert(
        Value::from("dns"),
        Value::Sequence(vec![Value::from(c.dns.clone())]),
    );
    let allowed_ips = if c.allowed_ips.is_empty() {
        &c.cidrs
    } else {
        &c.allowed_ips
    };
    map.insert(
        Value::from("allowed-ips"),
        Value::Sequence(allowed_ips.iter().cloned().map(Value::from).collect()),
    );
    Value::Mapping(map)
}
