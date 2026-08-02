//! 跨平台端口冲突释放（≡ Node ServiceManager.killPort）
//!
//! Unix：`lsof -ti :port` 解析 PID 后 `kill -9`。
//! Windows：`netstat -ano` 解析 LISTENING 行后 `taskkill /PID /F`。

use std::collections::HashSet;
use std::process::Command;

/// 释放占用指定端口的进程（返回值 true 表示有进程被终止）
pub fn release_port(port: u16) -> bool {
    #[cfg(windows)]
    {
        release_port_windows(port)
    }
    #[cfg(not(windows))]
    {
        release_port_unix(port)
    }
}

#[cfg(not(windows))]
fn release_port_unix(port: u16) -> bool {
    let out = match Command::new("lsof").args(["-ti", &format!(":{}", port)]).output() {
        Ok(o) => o,
        Err(_) => return false,
    };
    if !out.status.success() {
        return false;
    }
    let pids = String::from_utf8_lossy(&out.stdout);
    if pids.trim().is_empty() {
        return false;
    }
    let mut killed = false;
    for pid in pids.lines() {
        let pid = pid.trim();
        if pid.is_empty() {
            continue;
        }
        if Command::new("kill")
            .args(["-9", pid])
            .status()
            .map(|s| s.success())
            .unwrap_or(false)
        {
            killed = true;
        }
    }
    killed
}

#[cfg(windows)]
fn release_port_windows(port: u16) -> bool {
    let out = match Command::new("cmd").args(["/c", "netstat -ano"]).output() {
        Ok(o) => o,
        Err(_) => return false,
    };
    let content = String::from_utf8_lossy(&out.stdout);
    let mut pids: HashSet<String> = HashSet::new();
    let pat = format!(":{}", port);
    for line in content.lines() {
        let line = line.trim();
        if !line.contains("LISTENING") {
            continue;
        }
        if !line.contains(&pat) {
            continue;
        }
        let parts: Vec<&str> = line.split_whitespace().collect();
        if let Some(pid) = parts.last() {
            if pid.parse::<u32>().is_ok() {
                pids.insert(pid.to_string());
            }
        }
    }
    if pids.is_empty() {
        return false;
    }
    let mut killed = false;
    for pid in pids {
        if Command::new("taskkill")
            .args(["/PID", &pid, "/F"])
            .status()
            .map(|s| s.success())
            .unwrap_or(false)
        {
            killed = true;
        }
    }
    killed
}
