use serde_json::Value;

use crate::error::{AppError, AppResult};
use crate::settings::AppSettings;

fn auth_client(_settings: &AppSettings) -> reqwest::Client {
    reqwest::Client::new()
}

async fn send_json(
    settings: &AppSettings,
    method: reqwest::Method,
    path: &str,
    body: Option<Value>,
) -> AppResult<Value> {
    let client = auth_client(settings);
    let url = format!("{}{}", settings.api_base().trim_end_matches('/'), path);
    let mut req = client.request(method, url);
    if !settings.secret.is_empty() {
        req = req.header("Authorization", format!("Bearer {}", settings.secret));
    }
    if let Some(b) = body {
        req = req.json(&b);
    }
    let resp = req.send().await?;
    let status = resp.status();
    let text = resp.text().await.unwrap_or_default();
    if !status.is_success() {
        return Err(AppError::from(format!(
            "mihomo API {path} failed: {status} {text}"
        )));
    }
    if text.trim().is_empty() {
        return Ok(Value::Null);
    }
    Ok(serde_json::from_str(&text).unwrap_or(Value::String(text)))
}

pub async fn get_version(settings: &AppSettings) -> AppResult<Value> {
    send_json(settings, reqwest::Method::GET, "/version", None).await
}

/// Query public egress IP. When `via_proxy` is true, go through local mixed-port.
pub async fn fetch_outbound_ip(mixed_port: u16, via_proxy: bool) -> AppResult<String> {
    let mut builder = reqwest::Client::builder().timeout(std::time::Duration::from_secs(8));
    if via_proxy {
        let proxy = reqwest::Proxy::all(format!("http://127.0.0.1:{mixed_port}"))
            .map_err(|e| AppError::from(format!("代理设置失败: {e}")))?;
        builder = builder.proxy(proxy);
    } else {
        builder = builder.no_proxy();
    }
    let client = builder
        .build()
        .map_err(|e| AppError::from(format!("创建客户端失败: {e}")))?;

    // Prefer plain-text endpoints; fall through on failure.
    let urls = [
        "https://api.ipify.org",
        "https://api.ip.sb/ip",
        "https://ifconfig.me/ip",
    ];
    let mut last_err = String::from("查询出口 IP 失败");
    for url in urls {
        match client.get(url).send().await {
            Ok(resp) if resp.status().is_success() => {
                let text = resp.text().await.unwrap_or_default();
                let ip = text.trim().trim_matches('"').to_string();
                if !ip.is_empty() && ip.len() < 64 {
                    return Ok(ip);
                }
                last_err = format!("无效响应: {text}");
            }
            Ok(resp) => {
                last_err = format!("{url} -> {}", resp.status());
            }
            Err(e) => {
                last_err = e.to_string();
            }
        }
    }
    Err(AppError::from(last_err))
}

/// Best-effort LAN IPv4 (skip loopback / link-local).
pub fn local_ipv4() -> Option<String> {
    use std::net::{IpAddr, UdpSocket};
    let socket = UdpSocket::bind("0.0.0.0:0").ok()?;
    // No packets are sent; OS picks the interface used to reach a public address.
    socket.connect("8.8.8.8:80").ok()?;
    match socket.local_addr().ok()?.ip() {
        IpAddr::V4(ip) if !ip.is_loopback() && !ip.is_link_local() => Some(ip.to_string()),
        _ => None,
    }
}

pub async fn get_proxies(settings: &AppSettings) -> AppResult<Value> {
    send_json(settings, reqwest::Method::GET, "/proxies", None).await
}

pub async fn get_rules(settings: &AppSettings) -> AppResult<Value> {
    send_json(settings, reqwest::Method::GET, "/rules", None).await
}

pub async fn select_proxy(settings: &AppSettings, group: &str, name: &str) -> AppResult<()> {
    let path = format!("/proxies/{}", urlencoding_path(group));
    send_json(
        settings,
        reqwest::Method::PUT,
        &path,
        Some(serde_json::json!({ "name": name })),
    )
    .await?;
    Ok(())
}

pub async fn get_connections(settings: &AppSettings) -> AppResult<Value> {
    send_json(settings, reqwest::Method::GET, "/connections", None).await
}

pub async fn close_all_connections(settings: &AppSettings) -> AppResult<()> {
    send_json(settings, reqwest::Method::DELETE, "/connections", None).await?;
    Ok(())
}

pub async fn proxy_delay(
    settings: &AppSettings,
    name: &str,
    timeout_ms: u64,
    test_url: Option<&str>,
) -> AppResult<u32> {
    let url = test_url
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .unwrap_or("https://www.gstatic.com/generate_204");
    let path = format!(
        "/proxies/{}/delay?timeout={}&url={}",
        urlencoding_path(name),
        timeout_ms,
        urlencoding_path(url)
    );
    let value = send_json(settings, reqwest::Method::GET, &path, None).await?;
    let delay = value.get("delay").and_then(|v| v.as_u64()).unwrap_or(0) as u32;
    if delay == 0 {
        return Err(AppError::from("测速超时或节点不可用"));
    }
    Ok(delay)
}

/// TCP connect via local mixed-port (SOCKS5). Used for company VPN health checks:
/// WireGuard endpoints are not HTTPS sites, and gstatic is outside AllowedIPs.
pub async fn socks5_connect_ms(
    mixed_port: u16,
    dest_host: &str,
    dest_port: u16,
    timeout_ms: u64,
) -> AppResult<u32> {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::net::TcpStream;

    let host = dest_host.trim();
    if host.is_empty() || dest_port == 0 {
        return Err(AppError::from("探测目标无效"));
    }
    let timeout = std::time::Duration::from_millis(timeout_ms.max(500));
    let started = std::time::Instant::now();
    let result = tokio::time::timeout(timeout, async {
        let mut stream = TcpStream::connect(("127.0.0.1", mixed_port)).await?;
        stream.write_all(&[0x05, 0x01, 0x00]).await?;
        let mut greet = [0u8; 2];
        stream.read_exact(&mut greet).await?;
        if greet[0] != 0x05 || greet[1] != 0x00 {
            return Err(std::io::Error::other("SOCKS5 握手失败"));
        }

        let mut req = vec![0x05, 0x01, 0x00];
        if let Ok(ip) = host.parse::<std::net::Ipv4Addr>() {
            req.push(0x01);
            req.extend_from_slice(&ip.octets());
        } else if let Ok(ip) = host.parse::<std::net::Ipv6Addr>() {
            req.push(0x04);
            req.extend_from_slice(&ip.octets());
        } else {
            let bytes = host.as_bytes();
            if bytes.len() > 255 {
                return Err(std::io::Error::other("主机名过长"));
            }
            req.push(0x03);
            req.push(bytes.len() as u8);
            req.extend_from_slice(bytes);
        }
        req.push((dest_port >> 8) as u8);
        req.push((dest_port & 0xff) as u8);
        stream.write_all(&req).await?;

        let mut head = [0u8; 4];
        stream.read_exact(&mut head).await?;
        if head[0] != 0x05 || head[1] != 0x00 {
            return Err(std::io::Error::other(format!(
                "SOCKS5 连接失败 code={}",
                head[1]
            )));
        }
        match head[3] {
            0x01 => {
                let mut skip = [0u8; 4 + 2];
                stream.read_exact(&mut skip).await?;
            }
            0x03 => {
                let mut len = [0u8; 1];
                stream.read_exact(&mut len).await?;
                let mut skip = vec![0u8; len[0] as usize + 2];
                stream.read_exact(&mut skip).await?;
            }
            0x04 => {
                let mut skip = [0u8; 16 + 2];
                stream.read_exact(&mut skip).await?;
            }
            _ => return Err(std::io::Error::other("SOCKS5 地址类型未知")),
        }
        Ok::<(), std::io::Error>(())
    })
    .await;

    match result {
        Ok(Ok(())) => {
            let ms = started.elapsed().as_millis() as u32;
            Ok(ms.max(1))
        }
        Ok(Err(e)) => Err(AppError::from(format!("公司网络探测失败: {e}"))),
        Err(_) => Err(AppError::from("公司网络探测超时")),
    }
}

fn urlencoding_path(value: &str) -> String {
    let mut out = String::new();
    for byte in value.as_bytes() {
        match *byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(*byte as char);
            }
            _ => out.push_str(&format!("%{byte:02X}")),
        }
    }
    out
}
