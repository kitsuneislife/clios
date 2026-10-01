//! `clios status`: estado do sistema para a barra, em JSON de uma linha. A shell chama a cada
//! poucos segundos; por isso é um binário Rust de partida rápida e não um script.

use std::fs;
use std::path::Path;
use std::process::Command;

use anyhow::Result;
use serde::Serialize;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Net {
    /// `wifi`, `eth` ou `none`.
    pub kind: &'static str,
    /// SSID quando é wi-fi e deu para descobrir; senão vazio.
    pub label: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Iface {
    pub name: String,
    pub up: bool,
    pub wireless: bool,
}

/// Interfaces que não representam "estar online": loopback, contêineres, VPNs, bridges.
fn is_virtual(name: &str) -> bool {
    name == "lo"
        || ["veth", "docker", "br-", "virbr", "tun", "tap", "wg", "vmnet", "podman", "zt"]
            .iter()
            .any(|p| name.starts_with(p))
}

/// A interface da rota padrão, de `/proc/net/route`.
pub fn default_route_iface(route: &str) -> Option<String> {
    route.lines().skip(1).find_map(|l| {
        let mut f = l.split_whitespace();
        let (iface, dest) = (f.next()?, f.next()?);
        (dest == "00000000").then(|| iface.to_string())
    })
}

/// Escolhe qual interface representa a conexão: a da rota padrão se estiver de pé;
/// senão cabo antes de wi-fi (cabo costuma ser a conexão pretendida).
pub fn pick<'a>(ifaces: &'a [Iface], default: Option<&str>) -> Option<&'a Iface> {
    let up = || ifaces.iter().filter(|i| i.up && !is_virtual(&i.name));
    default
        .and_then(|d| up().find(|i| i.name == d))
        .or_else(|| up().find(|i| !i.wireless))
        .or_else(|| up().find(|i| i.wireless))
}

/// `SSID: Casa 5G` da saída de `iw dev <if> link`.
pub fn parse_iw_ssid(out: &str) -> Option<String> {
    out.lines().find_map(|l| l.trim().strip_prefix("SSID:").map(|s| s.trim().to_string())).filter(|s| !s.is_empty())
}

fn read_ifaces(sys_net: &Path) -> Vec<Iface> {
    let Ok(dir) = fs::read_dir(sys_net) else { return Vec::new() };
    let mut v: Vec<Iface> = dir
        .flatten()
        .map(|e| {
            let p = e.path();
            let state = fs::read_to_string(p.join("operstate")).unwrap_or_default();
            Iface {
                name: e.file_name().to_string_lossy().into_owned(),
                // wi-fi conectado costuma reportar "up"; "unknown" é comum em USB-ethernet.
                up: matches!(state.trim(), "up" | "unknown"),
                wireless: p.join("wireless").exists() || p.join("phy80211").exists(),
            }
        })
        .collect();
    v.sort_by(|a, b| a.name.cmp(&b.name));
    v
}

pub fn net() -> Net {
    let ifaces = read_ifaces(Path::new("/sys/class/net"));
    let route = fs::read_to_string("/proc/net/route").unwrap_or_default();
    match pick(&ifaces, default_route_iface(&route).as_deref()) {
        None => Net { kind: "none", label: String::new() },
        Some(i) if !i.wireless => Net { kind: "eth", label: String::new() },
        Some(i) => {
            let ssid = Command::new("iw")
                .args(["dev", &i.name, "link"])
                .output()
                .ok()
                .and_then(|o| parse_iw_ssid(&String::from_utf8_lossy(&o.stdout)));
            Net { kind: "wifi", label: ssid.unwrap_or_default() }
        }
    }
}

pub fn run_net() -> Result<()> {
    println!("{}", serde_json::to_string(&net())?);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn i(name: &str, up: bool, wireless: bool) -> Iface {
        Iface { name: name.into(), up, wireless }
    }

    const ROUTE: &str = "Iface\tDestination\tGateway\tFlags\tRefCnt\tUse\tMetric\tMask\tMTU\tWindow\tIRTT\n\
wlan0\t00000000\t0100A8C0\t0003\t0\t0\t600\t00000000\t0\t0\t0\n\
wlan0\t0000A8C0\t00000000\t0001\t0\t0\t600\t00FFFFFF\t0\t0\t0\n";

    #[test]
    fn default_route_is_found() {
        assert_eq!(default_route_iface(ROUTE).as_deref(), Some("wlan0"));
        assert_eq!(default_route_iface("Iface\tDestination\n"), None);
        assert_eq!(default_route_iface(""), None);
    }

    #[test]
    fn offline_when_nothing_is_up() {
        assert_eq!(pick(&[i("lo", true, false), i("wlan0", false, true)], None), None);
    }

    #[test]
    fn virtual_interfaces_never_count_as_online() {
        let v = [i("lo", true, false), i("docker0", true, false), i("veth1a", true, false), i("tun0", true, false)];
        assert_eq!(pick(&v, None), None);
    }

    #[test]
    fn default_route_wins_over_everything() {
        let v = [i("enp3s0", true, false), i("wlan0", true, true)];
        assert_eq!(pick(&v, Some("wlan0")).unwrap().name, "wlan0");
    }

    #[test]
    fn without_a_route_cable_beats_wifi() {
        let v = [i("enp3s0", true, false), i("wlan0", true, true)];
        assert_eq!(pick(&v, None).unwrap().name, "enp3s0");
        let only_wifi = [i("enp3s0", false, false), i("wlan0", true, true)];
        assert_eq!(pick(&only_wifi, None).unwrap().name, "wlan0");
    }

    #[test]
    fn stale_default_route_to_a_down_interface_is_ignored() {
        let v = [i("enp3s0", false, false), i("wlan0", true, true)];
        assert_eq!(pick(&v, Some("enp3s0")).unwrap().name, "wlan0");
    }

    #[test]
    fn ssid_parsing() {
        let out = "Connected to aa:bb:cc:dd:ee:ff (on wlan0)\n\tSSID: Casa 5G\n\tfreq: 5180\n\tsignal: -52 dBm\n";
        assert_eq!(parse_iw_ssid(out).as_deref(), Some("Casa 5G"));
        assert_eq!(parse_iw_ssid("Not connected.\n"), None);
        assert_eq!(parse_iw_ssid("\tSSID: \n"), None);
    }

    #[test]
    fn json_shape_is_what_the_shell_expects() {
        let j = serde_json::to_string(&Net { kind: "wifi", label: "Casa".into() }).unwrap();
        assert_eq!(j, r#"{"kind":"wifi","label":"Casa"}"#);
    }
}
