use anyhow::Result;
use std::collections::HashMap;
use std::fs;
use std::time::Duration;

#[derive(Clone, Debug, Default)]
pub struct ProcessInfo {
    pub pid: i32,
    pub cmd: String,
    pub rss_kb: u64,
}

#[derive(Clone, Debug)]
pub struct SystemSnapshot {
    pub cpu_usage: f64, // aggregate 0.0 - 1.0
    pub per_cpu_usage: Vec<f64>,
    pub load_avg: String,
    pub mem_total_kb: u64,
    pub mem_available_kb: u64,
    pub processes: Vec<ProcessInfo>,
}

impl Default for SystemSnapshot {
    fn default() -> Self {
        Self {
            cpu_usage: 0.0,
            per_cpu_usage: Vec::new(),
            load_avg: String::new(),
            mem_total_kb: 0,
            mem_available_kb: 0,
            processes: Vec::new(),
        }
    }
}

fn read_proc_stat_percpu() -> Result<Vec<(u64, u64)>> {
    let s = fs::read_to_string("/proc/stat")?;
    let mut out = Vec::new();
    for line in s.lines() {
        if line.starts_with("cpu") {
            let parts: Vec<&str> = line.split_whitespace().collect();
            // parts[0] is "cpu" or "cpuN"
            if parts.len() >= 5 {
                let mut nums = Vec::new();
                for p in parts.iter().skip(1) {
                    if let Ok(v) = p.parse::<u64>() {
                        nums.push(v);
                    } else {
                        nums.push(0);
                    }
                }
                // user, nice, system, idle, iowait, irq, softirq, steal
                let user = *nums.get(0).unwrap_or(&0);
                let nice = *nums.get(1).unwrap_or(&0);
                let system = *nums.get(2).unwrap_or(&0);
                let idle = *nums.get(3).unwrap_or(&0);
                let iowait = *nums.get(4).unwrap_or(&0);
                let irq = *nums.get(5).unwrap_or(&0);
                let softirq = *nums.get(6).unwrap_or(&0);
                let steal = *nums.get(7).unwrap_or(&0);
                let idle_all = idle + iowait;
                let non_idle = user + nice + system + irq + softirq + steal;
                let total = idle_all + non_idle;
                out.push((total, idle_all));
            }
        } else {
            // we only want the cpu lines at top
            break;
        }
    }
    if out.is_empty() {
        Err(anyhow::anyhow!("failed to parse /proc/stat"))
    } else {
        Ok(out)
    }
}

fn read_meminfo() -> Result<HashMap<String, u64>> {
    let s = fs::read_to_string("/proc/meminfo")?;
    let mut map = HashMap::new();
    for line in s.lines() {
        if let Some((k, v)) = line.split_once(':') {
            let val_part = v.trim().split_whitespace().next().unwrap_or("0");
            if let Ok(vv) = val_part.parse::<u64>() {
                map.insert(k.to_string(), vv);
            }
        }
    }
    Ok(map)
}

fn read_processes(limit: usize) -> Vec<ProcessInfo> {
    let mut procs = Vec::new();
    if let Ok(entries) = fs::read_dir("/proc") {
        for entry in entries.flatten() {
            if let Ok(fname) = entry.file_name().into_string() {
                if let Ok(pid) = fname.parse::<i32>() {
                    // read cmdline
                    let cmd = fs::read_to_string(format!("/proc/{}/cmdline", pid))
                        .unwrap_or_default()
                        .replace('\0', " ")
                        .trim()
                        .to_string();
                    // fallback to comm
                    let cmd = if cmd.is_empty() {
                        fs::read_to_string(format!("/proc/{}/comm", pid))
                            .unwrap_or_default()
                            .trim()
                            .to_string()
                    } else {
                        cmd
                    };

                    // read VmRSS from status
                    let mut rss_kb = 0u64;
                    if let Ok(status) = fs::read_to_string(format!("/proc/{}/status", pid)) {
                        for line in status.lines() {
                            if line.starts_with("VmRSS:") {
                                if let Some(val) = line.split_whitespace().nth(1) {
                                    rss_kb = val.parse().unwrap_or(0);
                                }
                                break;
                            }
                        }
                    }

                    procs.push(ProcessInfo { pid, cmd, rss_kb });
                }
            }
        }
    }
    // sort by rss desc
    procs.sort_by_key(|p| std::cmp::Reverse(p.rss_kb));
    procs.truncate(limit);
    procs
}

/// Collect a snapshot based on /proc data
pub fn collect_snapshot(prev: Option<Vec<(u64, u64)>>) -> Result<(SystemSnapshot, Vec<(u64, u64)>)> {
    let stats = read_proc_stat_percpu()?; // stats[0] is aggregate

    // compute per-core usage (including aggregate)
    let mut per_cpu_usage = Vec::new();
    if let Some(prev_stats) = prev {
        if prev_stats.len() == stats.len() {
            for (i, (total, idle)) in stats.iter().enumerate() {
                let (ptotal, pidle) = prev_stats[i];
                let totald = total.saturating_sub(ptotal) as f64;
                let idled = idle.saturating_sub(pidle) as f64;
                let usage = if totald > 0.0 { 1.0 - (idled / totald) } else { 0.0 };
                per_cpu_usage.push(usage.max(0.0).min(1.0));
            }
        }
    }

    // aggregate usage fallback
    let cpu_usage = per_cpu_usage.get(0).copied().unwrap_or(0.0);

    let memmap = read_meminfo().unwrap_or_default();
    let mem_total_kb = *memmap.get("MemTotal").unwrap_or(&0);
    let mem_available_kb = *memmap.get("MemAvailable").unwrap_or(&0);

    // loadavg
    let load_avg = fs::read_to_string("/proc/loadavg").unwrap_or_default();
    let load_avg = load_avg.split_whitespace().take(3).collect::<Vec<_>>().join(" ");

    let processes = read_processes(20);

    let snap = SystemSnapshot {
        cpu_usage,
        per_cpu_usage,
        load_avg,
        mem_total_kb,
        mem_available_kb,
        processes,
    };

    Ok((snap, stats))
}

#[derive(Clone, Debug, Default)]
pub struct ProcessDetails {
    pub pid: i32,
    pub cmdline: String,
    pub exe: String,
    pub cwd: String,
    pub uid: Option<u32>,
    pub gid: Option<u32>,
    pub threads: Option<u32>,
    pub rss_kb: Option<u64>,
    pub open_fds: Option<usize>,
}

pub fn get_process_details(pid: i32) -> Result<ProcessDetails> {
    let base = format!("/proc/{}", pid);
    let mut det = ProcessDetails { pid, ..Default::default() };

    // cmdline
    det.cmdline = fs::read_to_string(format!("{}/cmdline", base))
        .unwrap_or_default()
        .replace('\0', " ")
        .trim()
        .to_string();

    // exe and cwd
    det.exe = fs::read_link(format!("{}/exe", base)).ok().and_then(|p| p.into_os_string().into_string().ok()).unwrap_or_default();
    det.cwd = fs::read_link(format!("{}/cwd", base)).ok().and_then(|p| p.into_os_string().into_string().ok()).unwrap_or_default();

    // status for uid/gid/threads/Rss
    if let Ok(status) = fs::read_to_string(format!("{}/status", base)) {
        for line in status.lines() {
            if line.starts_with("Uid:") {
                if let Some(v) = line.split_whitespace().nth(1) {
                    det.uid = v.parse().ok();
                }
            } else if line.starts_with("Gid:") {
                if let Some(v) = line.split_whitespace().nth(1) {
                    det.gid = v.parse().ok();
                }
            } else if line.starts_with("Threads:") {
                if let Some(v) = line.split_whitespace().nth(1) {
                    det.threads = v.parse().ok();
                }
            } else if line.starts_with("VmRSS:") {
                if let Some(v) = line.split_whitespace().nth(1) {
                    det.rss_kb = v.parse().ok();
                }
            }
        }
    }

    // count open fds
    det.open_fds = fs::read_dir(format!("{}/fd", base)).ok().map(|rd| rd.count());

    Ok(det)
}

pub async fn updater(tx: tokio::sync::watch::Sender<SystemSnapshot>) {
    let mut prev: Option<Vec<(u64, u64)>> = None;
    let interval = Duration::from_millis(1000);
    loop {
        match collect_snapshot(prev.take()) {
            Ok((snap, p)) => {
                prev = Some(p);
                // ignore send errors
                let _ = tx.send(snap);
            }
            Err(_) => {
                // ignore transient errors
            }
        }
        tokio::time::sleep(interval).await;
    }
}
