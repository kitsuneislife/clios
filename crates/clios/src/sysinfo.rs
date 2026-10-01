//! Informações do sistema para o `clios fetch` e o guia de boas-vindas.
//! Só lê /proc e /etc: nada de processos externos além do `df`, e tudo que parseia é função pura.

use std::fs;
use std::path::Path;
use std::process::Command;

/// Valor de uma chave `CHAVE="valor"` (formato do /etc/os-release).
pub fn parse_os_release(text: &str, key: &str) -> Option<String> {
    text.lines().find_map(|l| {
        let (k, v) = l.split_once('=')?;
        (k == key).then(|| v.trim().trim_matches('"').to_string())
    })
}

/// `(usada, total)` em bytes, a partir do /proc/meminfo.
pub fn parse_meminfo(text: &str) -> Option<(u64, u64)> {
    let kb = |key: &str| -> Option<u64> {
        text.lines().find_map(|l| l.strip_prefix(key)?.trim_start_matches(':').split_whitespace().next()?.parse().ok())
    };
    let total = kb("MemTotal")?;
    let avail = kb("MemAvailable")?;
    Some((total.saturating_sub(avail) * 1024, total * 1024))
}

pub fn parse_uptime(text: &str) -> Option<u64> {
    text.split_whitespace().next()?.parse::<f64>().ok().map(|s| s as u64)
}

/// `2d 3h`, `3h 12m`, `12m`, `menos de 1m`.
pub fn fmt_duration(secs: u64) -> String {
    let (d, h, m) = (secs / 86_400, secs % 86_400 / 3_600, secs % 3_600 / 60);
    match (d, h, m) {
        (0, 0, 0) => "menos de 1m".into(),
        (0, 0, m) => format!("{m}m"),
        (0, h, m) => format!("{h}h {m}m"),
        (d, h, _) => format!("{d}d {h}h"),
    }
}

pub fn fmt_bytes(b: u64) -> String {
    const GIB: f64 = 1024.0 * 1024.0 * 1024.0;
    let g = b as f64 / GIB;
    if g >= 100.0 { format!("{g:.0} GiB") } else { format!("{g:.1} GiB") }
}

/// Primeiro `model name` do /proc/cpuinfo, sem a propaganda do fabricante.
pub fn parse_cpu(text: &str) -> Option<String> {
    let name = text.lines().find_map(|l| l.strip_prefix("model name")?.split_once(':').map(|(_, v)| v.trim()))?;
    let mut s = name.to_string();
    for noise in ["(R)", "(TM)", "(tm)", "CPU ", " Processor", "with Radeon Graphics"] {
        s = s.replace(noise, "");
    }
    // "Intel Core i7-1165G7 @ 2.80GHz" fica com o modelo e a frequência.
    Some(s.split_whitespace().collect::<Vec<_>>().join(" "))
}

/// `(usado, total)` em bytes de `df -B1 --output=used,size /`.
pub fn parse_df(text: &str) -> Option<(u64, u64)> {
    let line = text.lines().nth(1)?;
    let mut it = line.split_whitespace();
    Some((it.next()?.parse().ok()?, it.next()?.parse().ok()?))
}

/// Quantos pacotes o pacman tem instalados (um diretório por pacote).
pub fn count_packages(dir: &Path) -> Option<usize> {
    let n = fs::read_dir(dir).ok()?.flatten().filter(|e| e.path().is_dir()).count();
    (n > 0).then_some(n)
}

#[derive(Debug, Clone, Default)]
pub struct Info {
    pub user: String,
    pub host: String,
    pub os: String,
    pub kernel: String,
    pub uptime: Option<u64>,
    pub cpu: Option<String>,
    pub mem: Option<(u64, u64)>,
    pub disk: Option<(u64, u64)>,
    pub packages: Option<usize>,
    pub shell: String,
    pub wm: String,
}

pub fn gather() -> Info {
    let read = |p: &str| fs::read_to_string(p).unwrap_or_default();
    let env = |k: &str| std::env::var(k).ok().filter(|v| !v.is_empty());
    let os_release = read("/etc/os-release");
    let shell = env("SHELL").map(|s| s.rsplit('/').next().unwrap_or("").to_string()).unwrap_or_default();
    Info {
        user: env("USER").or_else(|| env("LOGNAME")).unwrap_or_default(),
        host: read("/proc/sys/kernel/hostname").trim().to_string(),
        os: parse_os_release(&os_release, "PRETTY_NAME").unwrap_or_else(|| "Linux".into()),
        kernel: read("/proc/sys/kernel/osrelease").trim().to_string(),
        uptime: parse_uptime(&read("/proc/uptime")),
        cpu: parse_cpu(&read("/proc/cpuinfo")),
        mem: parse_meminfo(&read("/proc/meminfo")),
        disk: Command::new("df")
            .args(["-B1", "--output=used,size", "/"])
            .output()
            .ok()
            .and_then(|o| parse_df(&String::from_utf8_lossy(&o.stdout))),
        packages: count_packages(Path::new("/var/lib/pacman/local")),
        shell,
        wm: env("XDG_CURRENT_DESKTOP").unwrap_or_default(),
    }
}

/// Saudação pela hora local (0..=23).
pub fn greeting(hour: u32) -> &'static str {
    match hour {
        5..=11 => "bom dia",
        12..=17 => "boa tarde",
        _ => "boa noite",
    }
}

/// A hora local, vinda do `date` (a std só conhece UTC). `None` se o `date` falhar.
pub fn local_hour() -> Option<u32> {
    let out = Command::new("date").arg("+%H").output().ok()?;
    String::from_utf8_lossy(&out.stdout).trim().parse().ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn os_release_strips_quotes() {
        let t = "NAME=\"Arch Linux\"\nPRETTY_NAME=\"Arch Linux\"\nID=arch\n";
        assert_eq!(parse_os_release(t, "PRETTY_NAME").as_deref(), Some("Arch Linux"));
        assert_eq!(parse_os_release(t, "ID").as_deref(), Some("arch"));
        assert_eq!(parse_os_release(t, "VERSION"), None);
    }

    #[test]
    fn meminfo_gives_used_and_total() {
        let t = "MemTotal:       16384000 kB\nMemFree:         1000000 kB\nMemAvailable:   10384000 kB\n";
        assert_eq!(parse_meminfo(t), Some((6_000_000 * 1024, 16_384_000 * 1024)));
        assert_eq!(parse_meminfo("nada"), None);
    }

    #[test]
    fn durations_read_like_a_person() {
        assert_eq!(fmt_duration(5), "menos de 1m");
        assert_eq!(fmt_duration(12 * 60), "12m");
        assert_eq!(fmt_duration(3 * 3600 + 12 * 60), "3h 12m");
        assert_eq!(fmt_duration(2 * 86_400 + 3 * 3600 + 59 * 60), "2d 3h");
        assert_eq!(parse_uptime("12345.67 98765.43\n"), Some(12345));
    }

    #[test]
    fn byte_formatting() {
        assert_eq!(fmt_bytes(1536 * 1024 * 1024), "1.5 GiB");
        assert_eq!(fmt_bytes(512 * 1024 * 1024 * 1024), "512 GiB");
    }

    #[test]
    fn cpu_name_loses_the_marketing() {
        let t = "processor\t: 0\nmodel name\t: 11th Gen Intel(R) Core(TM) i7-1165G7 @ 2.80GHz\n";
        assert_eq!(parse_cpu(t).as_deref(), Some("11th Gen Intel Core i7-1165G7 @ 2.80GHz"));
        let r = "model name\t: AMD Ryzen 7 5800X 8-Core Processor\n";
        assert_eq!(parse_cpu(r).as_deref(), Some("AMD Ryzen 7 5800X 8-Core"));
    }

    #[test]
    fn df_output() {
        let t = "        Used        1B-blocks\n 53687091200 512110190592\n";
        assert_eq!(parse_df(t), Some((53_687_091_200, 512_110_190_592)));
        assert_eq!(parse_df("so uma linha"), None);
    }

    #[test]
    fn package_count_ignores_files() {
        let d = std::env::temp_dir().join(format!("clios-pk-{}", std::process::id()));
        let _ = fs::remove_dir_all(&d);
        fs::create_dir_all(d.join("a-1.0-1")).unwrap();
        fs::create_dir_all(d.join("b-2.0-1")).unwrap();
        fs::write(d.join("ALPM_DB_VERSION"), "9").unwrap();
        assert_eq!(count_packages(&d), Some(2));
        assert_eq!(count_packages(&d.join("nao-existe")), None);
    }

    #[test]
    fn greeting_by_hour() {
        assert_eq!(greeting(7), "bom dia");
        assert_eq!(greeting(14), "boa tarde");
        assert_eq!(greeting(21), "boa noite");
        assert_eq!(greeting(3), "boa noite");
    }
}
