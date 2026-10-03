//! `clios battery`: a saúde da bateria e o limite de carga.
//!
//! Bateria de lítio que passa o dia na tomada a 100% envelhece mais rápido. Boa parte dos notebooks deixa o kernel
//! parar a carga antes (`charge_control_end_threshold`), mas o valor some no reinício. O CLIOS grava o limite num
//! arquivo do systemd-tmpfiles, que o reaplica a cada boot.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use anyhow::{Context, Result, bail};

use crate::ui;

const TMPFILES: &str = "/etc/tmpfiles.d/clios-battery.conf";

#[derive(Debug, Clone, PartialEq)]
pub struct Battery {
    pub name: String,
    pub dir: PathBuf,
    /// 0..100
    pub percent: Option<u32>,
    pub status: String,
    /// Capacidade de hoje em relação à de fábrica, 0..100.
    pub health: Option<f64>,
    pub cycles: Option<u32>,
    pub limit: Option<u32>,
}

fn read(dir: &Path, f: &str) -> Option<String> {
    fs::read_to_string(dir.join(f)).ok().map(|s| s.trim().to_string()).filter(|s| !s.is_empty())
}

fn num(dir: &Path, f: &str) -> Option<f64> {
    read(dir, f)?.parse().ok()
}

/// Lê uma bateria de `/sys/class/power_supply/BATx`. Os nomes variam: energia (µWh) ou carga (µAh).
pub fn read_battery(dir: &Path) -> Option<Battery> {
    if read(dir, "type").as_deref() != Some("Battery") {
        return None;
    }
    let health = match (num(dir, "energy_full"), num(dir, "energy_full_design")) {
        (Some(f), Some(d)) if d > 0.0 => Some(f / d * 100.0),
        _ => match (num(dir, "charge_full"), num(dir, "charge_full_design")) {
            (Some(f), Some(d)) if d > 0.0 => Some(f / d * 100.0),
            _ => None,
        },
    };
    Some(Battery {
        name: dir.file_name()?.to_string_lossy().into_owned(),
        dir: dir.to_path_buf(),
        percent: read(dir, "capacity").and_then(|s| s.parse().ok()),
        status: read(dir, "status").unwrap_or_default(),
        health,
        // alguns firmwares respondem 0 por não saberem
        cycles: read(dir, "cycle_count").and_then(|s| s.parse().ok()).filter(|&c| c > 0),
        limit: read(dir, "charge_control_end_threshold").and_then(|s| s.parse().ok()),
    })
}

pub fn batteries(root: &Path) -> Vec<Battery> {
    let Ok(rd) = fs::read_dir(root) else { return Vec::new() };
    let mut v: Vec<Battery> = rd.flatten().filter_map(|e| read_battery(&e.path())).collect();
    v.sort_by(|a, b| a.name.cmp(&b.name));
    v
}

/// O arquivo do tmpfiles que reaplica o limite em cada bateria que aceita.
pub fn tmpfiles(bats: &[Battery], limit: u32) -> String {
    let mut s = String::from("# CLIOS: limite de carga da bateria (clios battery limit). Apague para desfazer.\n");
    for b in bats.iter().filter(|b| b.limit.is_some()) {
        s.push_str(&format!("w {}/charge_control_end_threshold - - - - {limit}\n", b.dir.display()));
    }
    s
}

pub fn status_word(s: &str) -> &str {
    match s {
        "Charging" => "carregando",
        "Discharging" => "na bateria",
        "Full" => "cheia",
        "Not charging" => "parada (no limite ou na tomada)",
        _ => s,
    }
}

pub fn health_word(h: f64) -> &'static str {
    match h {
        h if h >= 90.0 => "ótima",
        h if h >= 80.0 => "boa",
        h if h >= 65.0 => "gasta",
        _ => "no fim",
    }
}

const ROOT: &str = "/sys/class/power_supply";

pub fn show() -> Result<()> {
    let bats = batteries(Path::new(ROOT));
    if bats.is_empty() {
        println!("nenhuma bateria (é um desktop?)");
        return Ok(());
    }
    for b in &bats {
        println!("{}", ui::bold(&b.name));
        if let Some(p) = b.percent {
            println!("  carga     {p}%  {}", ui::dim(status_word(&b.status)));
        }
        if let Some(h) = b.health {
            println!("  saúde     {h:.0}%  {}", ui::dim(&format!("{} · da capacidade de fábrica", health_word(h))));
        }
        if let Some(c) = b.cycles {
            println!("  ciclos    {c}");
        }
        match b.limit {
            Some(100) => println!("  limite    nenhum  {}", ui::dim("`clios battery limit 80` poupa a bateria")),
            Some(l) => println!("  limite    {l}%  {}", ui::dim("`clios battery limit off` desliga")),
            None => println!("  limite    {}", ui::dim("este notebook não deixa limitar a carga pelo kernel")),
        }
    }
    Ok(())
}

pub fn limit(arg: &str) -> Result<()> {
    let value = match arg {
        "off" | "desligar" | "100" => 100,
        n => n
            .parse::<u32>()
            .ok()
            .filter(|v| (50..=100).contains(v))
            .with_context(|| format!("limite {n:?} inválido: use um número de 50 a 100, ou off"))?,
    };
    let bats = batteries(Path::new(ROOT));
    if !bats.iter().any(|b| b.limit.is_some()) {
        bail!("nenhuma bateria aqui aceita limite de carga pelo kernel (charge_control_end_threshold)");
    }
    if value == 100 {
        let _ = Command::new("sudo").args(["rm", "-f", TMPFILES]).status();
        for b in bats.iter().filter(|b| b.limit.is_some()) {
            write_threshold(&b.dir, 100)?;
        }
        println!("sem limite: a bateria carrega até 100%");
        return Ok(());
    }
    let body = tmpfiles(&bats, value);
    let mut child = Command::new("sudo")
        .args(["tee", TMPFILES])
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .spawn()
        .context("sudo tee")?;
    std::io::Write::write_all(child.stdin.as_mut().context("stdin")?, body.as_bytes())?;
    if !child.wait()?.success() {
        bail!("não consegui gravar {TMPFILES}");
    }
    let applied = Command::new("sudo").args(["systemd-tmpfiles", "--create", TMPFILES]).status()?.success();
    if !applied {
        bail!("gravei {TMPFILES}, mas o systemd-tmpfiles não aplicou agora (vale a partir do próximo boot)");
    }
    println!("a carga para em {value}%, também depois de reiniciar. Para uma viagem: `clios battery limit off`.");
    Ok(())
}

fn write_threshold(dir: &Path, v: u32) -> Result<()> {
    let path = dir.join("charge_control_end_threshold");
    let mut child = Command::new("sudo")
        .arg("tee")
        .arg(&path)
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .spawn()
        .context("sudo tee")?;
    std::io::Write::write_all(child.stdin.as_mut().context("stdin")?, format!("{v}\n").as_bytes())?;
    if !child.wait()?.success() {
        bail!("não consegui escrever em {}", path.display());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fake(root: &Path, name: &str, files: &[(&str, &str)]) -> PathBuf {
        let d = root.join(name);
        fs::create_dir_all(&d).unwrap();
        for (f, v) in files {
            fs::write(d.join(f), format!("{v}\n")).unwrap();
        }
        d
    }

    fn root(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("clios-bat-{}-{name}", std::process::id()));
        let _ = fs::remove_dir_all(&d);
        d
    }

    #[test]
    fn energy_and_charge_batteries_both_report_health() {
        let r = root("both");
        fake(
            &r,
            "BAT0",
            &[
                ("type", "Battery"),
                ("capacity", "77"),
                ("status", "Discharging"),
                ("energy_full", "45000000"),
                ("energy_full_design", "50000000"),
                ("cycle_count", "312"),
                ("charge_control_end_threshold", "100"),
            ],
        );
        fake(
            &r,
            "BAT1",
            &[("type", "Battery"), ("charge_full", "3000"), ("charge_full_design", "4000"), ("cycle_count", "0")],
        );
        fake(&r, "AC", &[("type", "Mains"), ("online", "1")]);
        let b = batteries(&r);
        assert_eq!(b.len(), 2);
        assert_eq!((b[0].percent, b[0].cycles, b[0].limit), (Some(77), Some(312), Some(100)));
        assert!((b[0].health.unwrap() - 90.0).abs() < 1e-9);
        assert!((b[1].health.unwrap() - 75.0).abs() < 1e-9);
        assert_eq!((b[1].cycles, b[1].limit), (None, None));
    }

    #[test]
    fn the_tmpfiles_line_only_touches_batteries_that_accept_a_limit() {
        let r = root("tmp");
        fake(&r, "BAT0", &[("type", "Battery"), ("charge_control_end_threshold", "100")]);
        fake(&r, "BAT1", &[("type", "Battery")]);
        let body = tmpfiles(&batteries(&r), 80);
        assert!(body.contains(&format!("w {}/BAT0/charge_control_end_threshold - - - - 80\n", r.display())));
        assert!(!body.contains("BAT1"));
    }

    #[test]
    fn words() {
        assert_eq!(health_word(95.0), "ótima");
        assert_eq!(health_word(70.0), "gasta");
        assert_eq!(status_word("Not charging"), "parada (no limite ou na tomada)");
    }
}
