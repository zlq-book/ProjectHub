use std::{
    io::{Read, Write},
    path::{Path, PathBuf},
    process::{Command, Stdio},
    sync::{
        atomic::{AtomicBool, AtomicUsize, Ordering},
        Arc, Condvar, Mutex, OnceLock,
    },
    time::{Duration, Instant},
};

pub(super) struct Output {
    pub bytes: Vec<u8>,
    pub success: bool,
    pub truncated: bool,
    pub timed_out: bool,
    pub stderr: String,
}
struct Permit;
fn gate() -> &'static (Mutex<usize>, Condvar) {
    static GATE: OnceLock<(Mutex<usize>, Condvar)> = OnceLock::new();
    GATE.get_or_init(|| (Mutex::new(0), Condvar::new()))
}
impl Permit {
    fn acquire(deadline: Instant) -> Result<Self, String> {
        let (lock, signal) = gate();
        let mut count = lock.lock().map_err(|_| "Git 进程锁异常")?;
        while *count >= 2 {
            let remaining = deadline
                .checked_duration_since(Instant::now())
                .ok_or("Git 读取等待超时")?;
            count = signal
                .wait_timeout(count, remaining)
                .map_err(|_| "Git 进程锁异常")?
                .0;
        }
        *count += 1;
        Ok(Self)
    }
}
impl Drop for Permit {
    fn drop(&mut self) {
        let (lock, signal) = gate();
        if let Ok(mut count) = lock.lock() {
            *count -= 1;
            signal.notify_one();
        }
    }
}

pub(super) fn executable() -> Result<PathBuf, String> {
    let name = if cfg!(windows) { "git.exe" } else { "git" };
    std::env::var_os("PATH")
        .into_iter()
        .flat_map(|paths| std::env::split_paths(&paths).collect::<Vec<_>>())
        .filter(|path| path.is_absolute())
        .map(|path| path.join(name))
        .find(|path| path.is_file())
        .ok_or_else(|| "未找到 Git，请安装 Git 并将其加入 PATH".into())
}

fn drain(
    mut input: impl Read,
    cap: usize,
    total: Arc<AtomicUsize>,
    exceeded: Arc<AtomicBool>,
) -> Vec<u8> {
    let mut kept = Vec::new();
    let mut buffer = [0_u8; 8192];
    loop {
        let count = match input.read(&mut buffer) {
            Ok(0) | Err(_) => break,
            Ok(count) => count,
        };
        let previous = total.fetch_add(count, Ordering::Relaxed);
        let remaining = cap.saturating_sub(previous);
        kept.extend_from_slice(&buffer[..count.min(remaining)]);
        if count > remaining {
            exceeded.store(true, Ordering::Release);
        }
    }
    kept
}

// 所有生产调用均由白名单构造参数；独立排空两个管道，容量、排队和执行共同受截止时间约束。
pub(super) fn run(
    exe: &Path,
    directory: &Path,
    args: &[&str],
    deadline: Instant,
    cap: usize,
) -> Result<Output, String> {
    if matches!(args.first(), Some(&"status") | Some(&"diff")) {
        verify_filters(exe, directory, deadline)?;
    }
    run_input(exe, directory, args, deadline, cap, None)
}
fn verify_filters(exe: &Path, directory: &Path, deadline: Instant) -> Result<(), String> {
    // 仅在实际文件属性使用外部 clean/process 驱动时停止，不误拦全局未使用的 LFS 配置。
    let config = run_input(
        exe,
        directory,
        &[
            "config",
            "--null",
            "--get-regexp",
            "^filter\\..*\\.(clean|process)$",
        ],
        deadline,
        65536,
        None,
    )?;
    if config.timed_out || config.truncated || (!config.success && !config.stderr.trim().is_empty())
    {
        return Err("无法完整验证 Git 过滤器配置，已停止工作区读取".into());
    }
    let mut drivers = std::collections::HashSet::new();
    for record in config.bytes.split(|byte| *byte == 0) {
        if let Some(pos) = record.iter().position(|byte| *byte == b'\n') {
            let key = String::from_utf8_lossy(&record[..pos]);
            let value = String::from_utf8_lossy(&record[pos + 1..]);
            if !value.trim().is_empty() {
                if let Some(driver) = key.strip_prefix("filter.").and_then(|key| {
                    key.strip_suffix(".clean")
                        .or_else(|| key.strip_suffix(".process"))
                }) {
                    drivers.insert(driver.to_string());
                }
            }
        }
    }
    if drivers.is_empty() {
        return Ok(());
    }
    let files = run_input(
        exe,
        directory,
        &[
            "ls-files",
            "-z",
            "--cached",
            "--others",
            "--exclude-standard",
        ],
        deadline,
        4 * 1024 * 1024,
        None,
    )?;
    if !files.success || files.truncated || files.timed_out {
        return Err("无法完整验证 Git 文件过滤属性，已停止工作区读取".into());
    }
    let attrs = run_input(
        exe,
        directory,
        &["check-attr", "-z", "--stdin", "filter"],
        deadline,
        4 * 1024 * 1024,
        Some(&files.bytes),
    )?;
    if !attrs.success || attrs.truncated || attrs.timed_out {
        return Err("无法完整验证 Git 文件过滤属性，已停止工作区读取".into());
    }
    let fields = attrs.bytes.split(|byte| *byte == 0).collect::<Vec<_>>();
    for row in fields.chunks(3) {
        if row.len() == 3 && drivers.contains(String::from_utf8_lossy(row[2]).as_ref()) {
            return Err("文件使用 clean/process 过滤器；为避免读取时执行外部程序，暂不扫描工作区或生成其差异。提交历史仍可读取。".into());
        }
    }
    Ok(())
}
fn run_input(
    exe: &Path,
    directory: &Path,
    args: &[&str],
    deadline: Instant,
    cap: usize,
    input: Option<&[u8]>,
) -> Result<Output, String> {
    let _permit = Permit::acquire(deadline)?;
    if Instant::now() >= deadline {
        return Err("Git 读取超时".into());
    }
    let mut command = Command::new(exe);
    command
        .current_dir(directory)
        .args([
            "--no-pager",
            "--literal-pathspecs",
            "-c",
            "core.fsmonitor=false",
            "-c",
            "core.untrackedCache=false",
            "-c",
            "core.quotePath=false",
            "-c",
            "diff.external=",
            "-c",
            "log.showSignature=false",
            "-c",
            "commit.gpgsign=false",
            "-c",
            "tag.gpgsign=false",
            "-c",
            "maintenance.auto=false",
            "-c",
            "gc.auto=0",
            "-c",
            "protocol.allow=never",
            "-c",
            "submodule.recurse=false",
            "-c",
            "color.ui=false",
        ])
        .args(args)
        .stdin(if input.is_some() {
            Stdio::piped()
        } else {
            Stdio::null()
        })
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    for (key, _) in std::env::vars_os() {
        if key
            .to_string_lossy()
            .to_ascii_uppercase()
            .starts_with("GIT_")
        {
            command.env_remove(key);
        }
    }
    command
        .env("GIT_OPTIONAL_LOCKS", "0")
        .env("GIT_TERMINAL_PROMPT", "0")
        .env("GIT_NO_LAZY_FETCH", "1")
        .env("GIT_PAGER", "");
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        command.creation_flags(0x08000000);
    }
    let mut child = command
        .spawn()
        .map_err(|error| format!("无法启动 Git：{error}"))?;
    let writer = if let Some(input) = input {
        let bytes = input.to_vec();
        let mut stdin = child.stdin.take().ok_or("Git 输入管道不可用")?;
        Some(std::thread::spawn(move || {
            let _ = stdin.write_all(&bytes);
        }))
    } else {
        None
    };
    let total = Arc::new(AtomicUsize::new(0));
    let exceeded = Arc::new(AtomicBool::new(false));
    let stdout = child.stdout.take().ok_or("Git 输出管道不可用")?;
    let stderr = child.stderr.take().ok_or("Git 错误管道不可用")?;
    let out_total = total.clone();
    let out_exceeded = exceeded.clone();
    let out = std::thread::spawn(move || drain(stdout, cap, out_total, out_exceeded));
    let err_exceeded = exceeded.clone();
    let err = std::thread::spawn(move || drain(stderr, cap, total, err_exceeded));
    let mut timed_out = false;
    let exit = loop {
        match child.try_wait() {
            Ok(Some(exit)) => break Ok(exit),
            Ok(None) => {
                // 读取线程共享总字节数，因此 stderr 也不能无限增长。
                let over = exceeded.load(Ordering::Acquire);
                if Instant::now() >= deadline || over {
                    timed_out = Instant::now() >= deadline;
                    let _ = child.kill();
                    break child.wait();
                }
                std::thread::sleep(Duration::from_millis(5));
            }
            Err(error) => {
                let _ = child.kill();
                let _ = child.wait();
                break Err(error);
            }
        }
    };
    let bytes = out.join().map_err(|_| "Git 输出读取线程异常")?;
    let error_bytes = err.join().map_err(|_| "Git 错误读取线程异常")?;
    // 超限线程持续排空到进程退出，下面的长度只用于报告，超限终止由共享标记处理。
    if let Some(writer) = writer {
        let _ = writer.join();
    }
    let truncated = exceeded.load(Ordering::Acquire);
    let success = exit
        .map_err(|error| format!("Git 进程回收失败：{error}"))?
        .success();
    Ok(Output {
        bytes,
        success,
        truncated,
        timed_out,
        stderr: String::from_utf8_lossy(&error_bytes).into_owned(),
    })
}
