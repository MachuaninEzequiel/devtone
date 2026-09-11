use std::path::Path;

use devtone_core::{AgentKind, Lang};

pub fn lang_from_title(title: &str) -> Lang {
    let mut last = Lang::Other;
    for tok in title.split(|c: char| c.is_whitespace() || matches!(c, ',' | '|' | '—' | '-' | '(' | ')')) {
        let t = tok.trim_matches(|c: char| !c.is_ascii_alphanumeric() && c != '.' && c != '_');
        if let Some((_, ext)) = t.rsplit_once('.') {
            last = match ext.to_ascii_lowercase().as_str() {
                "rs" => Lang::Rs,
                "py" => Lang::Py,
                "ts" | "tsx" | "js" | "jsx" => Lang::Ts,
                "go" => Lang::Go,
                "sql" => Lang::Sql,
                _ => last,
            };
        }
    }
    last
}

pub fn agent_from_cmdline(cmdline: &str) -> Option<AgentKind> {
    let mut found = None;
    for arg in cmdline.split('\0') {
        if arg.is_empty() {
            continue;
        }
        let name = Path::new(arg)
            .file_name()
            .and_then(|s| s.to_str())
            .unwrap_or(arg);
        if name == "devtone" || name.starts_with("devtone-") {
            continue;
        }
        if let Some(k) = AgentKind::from_cli_name(name) {
            found = Some(k);
        }
        if name == "pi-coding-agent" {
            found = Some(AgentKind::Pi);
        }
    }
    found
}

pub fn is_terminal_class(class: &str) -> bool {
    let c = class.to_ascii_lowercase();
    [
        "konsole",
        "kitty",
        "ghostty",
        "alacritty",
        "wezterm",
        "foot",
        "gnome-terminal",
        "kgx",
        "org.kde.konsole",
        "org.wezfurlong.wezterm",
    ]
    .iter()
    .any(|n| c.contains(n))
}

pub fn foreground_agent(proc_root: &Path, pid: u32) -> Option<AgentKind> {
    let cmd = read_cmdline(proc_root, pid)?;
    if let Some(k) = agent_from_cmdline(&cmd) {
        return Some(k);
    }
    // Walk children in proc_root by scanning numeric dirs with matching PPid.
    let Ok(rd) = std::fs::read_dir(proc_root) else {
        return None;
    };
    for ent in rd.flatten() {
        let name = ent.file_name();
        let Some(child) = name.to_str().and_then(|s| s.parse::<u32>().ok()) else {
            continue;
        };
        if child == pid {
            continue;
        }
        if ppid_of(proc_root, child) == Some(pid) {
            if let Some(k) = read_cmdline(proc_root, child).and_then(|c| agent_from_cmdline(&c)) {
                return Some(k);
            }
        }
    }
    None
}

fn read_cmdline(proc_root: &Path, pid: u32) -> Option<String> {
    std::fs::read_to_string(proc_root.join(pid.to_string()).join("cmdline")).ok()
}

fn ppid_of(proc_root: &Path, pid: u32) -> Option<u32> {
    let st = std::fs::read_to_string(proc_root.join(pid.to_string()).join("stat")).ok()?;
    // pid (comm) state ppid ...
    let after = st.rsplit(')').next()?;
    let mut parts = after.split_whitespace();
    let _state = parts.next()?;
    parts.next()?.parse().ok()
}

pub struct WindowInfo {
    pub pid: u32,
    pub title: String,
    pub class: String,
}

pub fn linux_active_window() -> Option<WindowInfo> {
    kwin_active().or_else(hypr_active)
}

fn kwin_active() -> Option<WindowInfo> {
    use std::process::{Command, Stdio};
    use std::time::{Duration, Instant};
    let mut child = Command::new("kwindowprop")
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .ok()?;
    let t0 = Instant::now();
    loop {
        match child.try_wait() {
            Ok(Some(status)) if status.success() => break,
            Ok(Some(_)) => return None,
            Ok(None) if t0.elapsed() > Duration::from_millis(80) => {
                let _ = child.kill();
                let _ = child.wait();
                return None;
            }
            Ok(None) => std::thread::sleep(Duration::from_millis(5)),
            Err(_) => return None,
        }
    }
    let out = child.stdout.take()?;
    let mut buf = Vec::new();
    let _ = std::io::Read::read_to_end(&mut std::io::BufReader::new(out), &mut buf);
    if buf.is_empty() {
        return None;
    }
    parse_kwindowprop(&String::from_utf8_lossy(&buf))
}

pub fn parse_kwindowprop(text: &str) -> Option<WindowInfo> {
    let mut pid = None;
    let mut title = String::new();
    let mut class = String::new();
    for line in text.lines() {
        let Some((k, v)) = line.split_once(':') else {
            continue;
        };
        let v = v.trim();
        match k.trim() {
            "pid" => pid = v.parse().ok(),
            "caption" => title = v.to_string(),
            "resourceClass" => class = v.to_string(),
            _ => {}
        }
    }
    Some(WindowInfo {
        pid: pid?,
        title,
        class,
    })
}

fn hypr_active() -> Option<WindowInfo> {
    None
}

#[cfg(test)]
mod tests {
    use super::{agent_from_cmdline, lang_from_title, parse_kwindowprop};
    use devtone_core::{AgentKind, Lang};

    #[test]
    fn lang_from_title_uses_last_ext() {
        assert_eq!(lang_from_title("~/DevTone/crates/devtone-core/src/lib.rs"), Lang::Rs);
        assert_eq!(lang_from_title("main.ts — Konsole"), Lang::Ts);
    }

    #[test]
    fn kwindowprop_parses_alacritty() {
        let text = "pid: 19402\ncaption: ~/DevTone: pi\nresourceClass: Alacritty\n";
        let w = parse_kwindowprop(text).unwrap();
        assert_eq!(w.pid, 19402);
        assert_eq!(w.class, "Alacritty");
        assert!(w.title.contains("pi"));
    }

    #[test]
    fn cmdline_detects_pi_not_devtone() {
        assert_eq!(
            agent_from_cmdline("/usr/bin/node\0/home/x/.local/bin/pi\0"),
            Some(AgentKind::Pi)
        );
        assert_eq!(agent_from_cmdline("/home/x/devtone\0--headless\0"), None);
    }
}
