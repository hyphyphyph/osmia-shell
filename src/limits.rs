//! CPU and memory limits for a program, using cgroup v2 in the user's own
//! delegated subtree (no root needed). Each limited program gets a group under
//! `user@UID.service/osmia-limits/`. CPU uses `cpu.max` (percent of one core);
//! memory uses `memory.high` (the kernel reclaims and slows the program, it does
//! not kill it). Limits are kept in ~/.local/state/osmia/proc-limits.

use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;

#[derive(Clone, Copy, Default, PartialEq, Debug)]
pub struct Limit {
    /// Percent of the whole processor (100 = every core).
    pub cpu: Option<f32>,
    /// Megabytes.
    pub mem_mb: Option<u64>,
}

impl Limit {
    pub fn is_none(&self) -> bool {
        self.cpu.is_none() && self.mem_mb.is_none()
    }
}

#[derive(Default)]
pub struct Limits {
    pub set: HashMap<String, Limit>,
    /// Where each moved process came from, so it can go back.
    origin: HashMap<u32, String>,
    root: Option<PathBuf>,
}

fn state_file() -> Option<PathBuf> {
    let base = std::env::var_os("XDG_STATE_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".local/state")))?;
    Some(base.join("osmia/proc-limits"))
}

/// The `cpu.max` text for a share of the whole processor: quota per 100 ms period.
fn cpu_max(percent: f32) -> String {
    let quota = (percent as f64 / 100.0 * cores() as f64 * 100_000.0).round() as u64;
    format!("{} 100000", quota.max(1000))
}

fn cores() -> u64 {
    std::thread::available_parallelism().map(|n| n.get() as u64).unwrap_or(1)
}

fn safe(name: &str) -> String {
    let s: String = name.chars().map(|c| if c.is_ascii_alphanumeric() || c == '-' || c == '_' { c } else { '_' }).collect();
    format!("p-{s}")
}

/// `/sys/fs/cgroup/.../user@UID.service`, from our own cgroup path.
fn user_service_dir() -> Option<PathBuf> {
    let t = fs::read_to_string("/proc/self/cgroup").ok()?;
    let path = t.lines().find_map(|l| l.strip_prefix("0::"))?;
    let end = path.find(".service")? + ".service".len();
    let dir = PathBuf::from("/sys/fs/cgroup").join(path[..end].trim_start_matches('/'));
    dir.is_dir().then_some(dir)
}

impl Limits {
    pub fn load() -> Self {
        let mut l = Limits::default();
        if let Some(t) = state_file().and_then(|f| fs::read_to_string(f).ok()) {
            for line in t.lines() {
                let mut it = line.split('\t');
                let (Some(name), Some(c), Some(m)) = (it.next(), it.next(), it.next()) else { continue };
                let lim = Limit { cpu: c.parse().ok(), mem_mb: m.parse().ok() };
                if !name.is_empty() && !lim.is_none() {
                    l.set.insert(name.to_owned(), lim);
                }
            }
        }
        l
    }

    fn save(&self) {
        let Some(f) = state_file() else { return };
        if let Some(d) = f.parent() {
            let _ = fs::create_dir_all(d);
        }
        let mut out = String::new();
        for (n, l) in &self.set {
            let f = |v: Option<u64>| v.map(|v| v.to_string()).unwrap_or_default();
            let c = l.cpu.map(|c| c.to_string()).unwrap_or_default();
            out.push_str(&format!("{n}\t{c}\t{}\n", f(l.mem_mb)));
        }
        let _ = fs::write(f, out);
    }

    /// The group directory root, made on first use.
    fn root(&mut self) -> Result<PathBuf, String> {
        if let Some(r) = &self.root {
            return Ok(r.clone());
        }
        let base = user_service_dir().ok_or("No user cgroup (is this a systemd session?)")?;
        let r = base.join("osmia-limits");
        if !r.is_dir() {
            fs::create_dir(&r).map_err(|e| format!("Cannot create cgroup: {e}"))?;
        }
        fs::write(r.join("cgroup.subtree_control"), "+cpu +memory")
            .map_err(|e| format!("Cpu and memory limits are not delegated: {e}"))?;
        self.root = Some(r.clone());
        Ok(r)
    }

    /// Set or change one program's limit, then put its processes in the group.
    pub fn set(&mut self, name: &str, lim: Limit, pids: &[u32]) -> Result<(), String> {
        if lim.is_none() {
            self.clear(name, pids);
            return Ok(());
        }
        let dir = self.root()?.join(safe(name));
        if !dir.is_dir() {
            fs::create_dir(&dir).map_err(|e| format!("Cannot create cgroup: {e}"))?;
        }
        let w = |f: &str, v: String| fs::write(dir.join(f), v).map_err(|e| format!("{f}: {e}"));
        w("cpu.max", lim.cpu.map(|c| cpu_max(c)).unwrap_or_else(|| "max 100000".into()))?;
        w("memory.high", lim.mem_mb.map(|m| (m * 1024 * 1024).to_string()).unwrap_or_else(|| "max".into()))?;
        self.set.insert(name.to_owned(), lim);
        self.save();
        let failed = self.enforce_one(name, pids);
        if failed > 0 {
            return Err(format!("{failed} of {} processes could not be moved into the limit group. The rest are limited.", pids.len()));
        }
        Ok(())
    }

    /// Remove a program's limit and send its processes back where they were.
    pub fn clear(&mut self, name: &str, _pids: &[u32]) {
        self.set.remove(name);
        self.save();
        let Some(root) = self.root().ok() else { return };
        let dir = root.join(safe(name));
        let procs = fs::read_to_string(dir.join("cgroup.procs")).unwrap_or_default();
        for pid in procs.lines().filter_map(|l| l.parse::<u32>().ok()) {
            self.restore(pid, &root);
        }
        let _ = fs::remove_dir(&dir);
    }

    fn restore(&mut self, pid: u32, root: &std::path::Path) {
        let home = self.origin.remove(&pid).map(|o| PathBuf::from("/sys/fs/cgroup").join(o.trim_start_matches('/')));
        let target = home.filter(|h| h.is_dir()).unwrap_or_else(|| root.parent().map(|p| p.to_path_buf()).unwrap_or_default());
        let _ = fs::write(target.join("cgroup.procs"), pid.to_string());
    }

    /// Move processes into the program's group. Returns how many could not be moved.
    fn enforce_one(&mut self, name: &str, pids: &[u32]) -> usize {
        let Ok(root) = self.root() else { return pids.len() };
        let mut failed = 0;
        let dir = root.join(safe(name));
        let have = fs::read_to_string(dir.join("cgroup.procs")).unwrap_or_default();
        for &pid in pids {
            if have.lines().any(|l| l == pid.to_string()) {
                continue;
            }
            if let Some(cg) = fs::read_to_string(format!("/proc/{pid}/cgroup")).ok().and_then(|t| t.lines().find_map(|l| l.strip_prefix("0::").map(str::to_owned))) {
                self.origin.insert(pid, cg);
            }
            // A process that exited meanwhile is not a failure.
            if fs::write(dir.join("cgroup.procs"), pid.to_string()).is_err() && std::path::Path::new(&format!("/proc/{pid}")).exists() {
                failed += 1;
            }
        }
        failed
    }

    /// Remove groups with no limit and no processes left behind.
    fn remove_stale(&mut self) {
        let Some(root) = self.root.clone() else { return };
        let keep: Vec<String> = self.set.keys().map(|n| safe(n)).collect();
        let Ok(rd) = fs::read_dir(&root) else { return };
        for e in rd.flatten() {
            let n = e.file_name().to_string_lossy().into_owned();
            if n.starts_with("p-") && !keep.contains(&n) && fs::read_to_string(e.path().join("cgroup.procs")).map(|t| t.trim().is_empty()).unwrap_or(false) {
                let _ = fs::remove_dir(e.path());
            }
        }
    }

    /// Called at each sample: catch processes started since the last one.
    pub fn enforce(&mut self, all: &[crate::sys::Proc]) {
        self.remove_stale();
        if self.set.is_empty() {
            return;
        }
        let names: Vec<String> = self.set.keys().cloned().collect();
        for n in names {
            if let Some(p) = all.iter().find(|p| p.name == n) {
                let pids: Vec<u32> = p.members.iter().map(|m| m.0).collect();
                self.enforce_one(&n, &pids);
            }
        }
        self.origin.retain(|pid, _| std::path::Path::new(&format!("/proc/{pid}")).exists());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_are_safe_for_paths() {
        assert_eq!(safe("Firefox"), "p-Firefox");
        assert_eq!(safe("../etc/x y"), "p-___etc_x_y");
        assert!(!safe("a/b").contains('/'));
    }

    #[test]
    fn cpu_max_is_a_share_of_the_whole_processor() {
        let full = cpu_max(100.0);
        assert_eq!(full, format!("{} 100000", cores() * 100_000));
        assert!(cpu_max(0.0).starts_with("1000 "), "never below the kernel minimum");
    }

    #[test]
    fn limit_none() {
        assert!(Limit::default().is_none());
        assert!(!Limit { cpu: Some(1.5), mem_mb: None }.is_none());
    }

    /// Needs a systemd user session with delegated cpu and memory: `cargo test -- --ignored`.
    #[test]
    #[ignore]
    fn limits_cpu() {
        let mut child = std::process::Command::new("sh").args(["-c", "while :; do :; done"]).spawn().unwrap();
        let pid = child.id();
        let mut l = Limits::default();
        l.set("osmia-test", Limit { cpu: Some(2.0), mem_mb: Some(256) }, &[pid]).unwrap();
        let ticks = |p| fs::read_to_string(format!("/proc/{p}/stat")).unwrap().rsplit(')').next().unwrap().split_whitespace().skip(11).take(2).map(|v| v.parse::<u64>().unwrap()).sum::<u64>();
        std::thread::sleep(std::time::Duration::from_millis(500));
        let a = ticks(pid);
        std::thread::sleep(std::time::Duration::from_secs(3));
        let used = (ticks(pid) - a) as f64 / 100.0 / 3.0 * 100.0;
        println!("cpu used {used:.0}%");
        l.clear("osmia-test", &[pid]);
        let back = fs::read_to_string(format!("/proc/{pid}/cgroup")).unwrap();
        child.kill().unwrap();
        let _ = child.wait();
        assert!(used < 60.0, "used {used}");
        assert!(!back.contains("osmia-limits"), "{back}");
    }
}
