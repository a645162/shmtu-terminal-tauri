use local_ip_address::local_ip;
use serde::{Deserialize, Serialize};

use crate::protocol::PairCode;

/// QR 码载荷结构
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QRPayload {
    /// 所有本地 IP 地址
    pub ips: Vec<String>,
    /// 服务端口
    pub port: u16,
    /// 6 字符配对码
    pub pair_code: String,
    /// 协议版本
    pub version: u32,
}

/// P2P 发现信息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct P2PDiscoveryInfo {
    pub local_ips: Vec<String>,
    pub port: u16,
    pub pair_code: String,
}

/// 获取所有本地 IP 地址
pub fn get_local_ips() -> Vec<String> {
    let mut ips = Vec::new();

    // 获取主本地 IP
    match local_ip() {
        Ok(ip) => {
            ips.push(ip.to_string());
        }
        Err(e) => {
            tracing::warn!("[P2P] Failed to get local IP: {}", e);
        }
    }

    // 尝试获取所有网络接口的 IP
    match local_ip_address::list_afinet_netifas() {
        Ok(interfaces) => {
            for (_name, ip) in interfaces {
                let ip_str = ip.to_string();
                // 排除 loopback 和已添加的地址
                if !ip_str.starts_with("127.") && !ips.contains(&ip_str) {
                    ips.push(ip_str);
                }
            }
        }
        Err(e) => {
            tracing::debug!("[P2P] Failed to list network interfaces: {}", e);
        }
    }

    // 如果没有获取到任何 IP，添加 loopback 作为 fallback
    if ips.is_empty() {
        ips.push("127.0.0.1".to_string());
    }

    tracing::info!("[P2P] Local IPs: {:?}", ips);
    ips
}

/// 生成 QR 码载荷 JSON 字符串
pub fn generate_qr_payload(port: u16, pair_code: &PairCode) -> String {
    let ips = get_local_ips();
    let payload = QRPayload {
        ips,
        port,
        pair_code: pair_code.as_str().to_string(),
        version: 1,
    };
    serde_json::to_string(&payload).unwrap_or_default()
}

/// 生成 QR 码图片（SVG 格式字符串）
pub fn generate_qr_svg(data: &str) -> Result<String, Box<dyn std::error::Error>> {
    use qrcode::render::svg;
    use qrcode::QrCode;

    let code = QrCode::new(data)?;
    let svg_string = code
        .render()
        .min_dimensions(200, 200)
        .dark_color(svg::Color("#000000"))
        .light_color(svg::Color("#ffffff"))
        .build();
    Ok(svg_string)
}

/// 从 JSON 解析 QR 码载荷
pub fn parse_qr_payload(json: &str) -> Result<QRPayload, serde_json::Error> {
    serde_json::from_str(json)
}
