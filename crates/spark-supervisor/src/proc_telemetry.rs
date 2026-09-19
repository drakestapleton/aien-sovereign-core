use std::fs;
use std::path::Path;

#[derive(Debug, Clone, Default)]
pub struct ProcessStats {
    pub pid: u32,
    pub is_alive: bool,
    pub rss_bytes: u64,
    pub cmdline: String,
}

pub fn get_process_stats(pid: u32) -> ProcessStats {
    let proc_dir = format!("/proc/{}", pid);
    let path = Path::new(&proc_dir);
    if !path.exists() {
        return ProcessStats {
            pid,
            is_alive: false,
            rss_bytes: 0,
            cmdline: String::new(),
        };
    }

    let cmdline = fs::read_to_string(path.join("cmdline"))
        .map(|s| s.replace('\0', " ").trim().to_string())
        .unwrap_or_default();

    let mut rss_bytes = 0;
    if let Ok(statm) = fs::read_to_string(path.join("statm")) {
        let parts: Vec<&str> = statm.split_whitespace().collect();
        if parts.len() >= 2 {
            if let Ok(resident_pages) = parts[1].parse::<u64>() {
                // Typical page size is 4096 bytes on Linux arm64/x86_64
                rss_bytes = resident_pages * 4096;
            }
        }
    }

    ProcessStats {
        pid,
        is_alive: true,
        rss_bytes,
        cmdline,
    }
}
