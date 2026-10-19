use super::runner::{self, Output};
use serde::{Deserialize, Serialize};
use std::{
    collections::HashMap,
    fs,
    io::Read,
    path::{Component, Path, PathBuf},
    sync::{Mutex, OnceLock},
    time::{Duration, Instant},
};

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FileStatus {
    pub path: String,
    pub old_path: Option<String>,
    pub index_status: String,
    pub worktree_status: String,
    pub untracked: bool,
    pub conflicted: bool,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StatusSnapshot {
    pub repository: bool,
    pub repository_id: Option<String>,
    pub branch: Option<String>,
    pub head: Option<String>,
    pub read_at: String,
    pub complete: bool,
    pub warning: Option<String>,
    pub files: Vec<FileStatus>,
    pub latest_commit_at: Option<String>,
    pub shallow: bool,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Commit {
    pub sha: String,
    pub subject: String,
    pub message: String,
    pub author: String,
    pub author_time: String,
    pub commit_time: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct History {
    pub head: Option<String>,
    pub commits: Vec<Commit>,
    pub next_cursor: Option<String>,
    pub complete: bool,
    pub read_at: String,
    pub warning: Option<String>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CommitFile {
    pub path: String,
    pub old_path: Option<String>,
    pub status: String,
    pub additions: Option<u64>,
    pub deletions: Option<u64>,
    pub binary: bool,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CommitFiles {
    pub head: Option<String>,
    pub sha: String,
    pub first_parent: Option<String>,
    pub files: Vec<CommitFile>,
    pub complete: bool,
    pub warning: Option<String>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Diff {
    pub text: String,
    pub binary: bool,
    pub truncated: bool,
    pub warning: Option<String>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Activity {
    pub head: Option<String>,
    pub timestamps: Vec<String>,
    pub complete: bool,
    pub read_at: String,
    pub shallow: bool,
    pub warning: Option<String>,
}

const MAX_OUTPUT: usize = 4 * 1024 * 1024;
const MAX_DIFF: usize = 256 * 1024;
const MAX_STATUS: usize = 5000;
const MAX_FILES: usize = 2000;
const MAX_ACTIVITY: usize = 20000;
fn now() -> String {
    chrono::Utc::now().to_rfc3339()
}
fn utc(value: &str) -> Result<String, String> {
    chrono::DateTime::parse_from_rfc3339(value.trim())
        .map(|time| time.with_timezone(&chrono::Utc).to_rfc3339())
        .map_err(|_| "Git 提交时间格式无效".into())
}
fn decode(bytes: &[u8]) -> Result<String, String> {
    String::from_utf8(bytes.to_vec())
        .map_err(|_| "Git 返回非 UTF-8 数据，无法安全识别文件路径或提交内容".into())
}
fn warning(output: &Output) -> Option<String> {
    if output.timed_out {
        Some("Git 读取超时，结果不完整".into())
    } else if output.truncated {
        Some("Git 输出达到容量上限，结果不完整".into())
    } else if !output.success {
        Some(format!(
            "Git 读取失败：{}",
            output.stderr.chars().take(1000).collect::<String>()
        ))
    } else {
        None
    }
}
fn checked(output: Output) -> Result<Vec<u8>, String> {
    if let Some(message) = warning(&output) {
        Err(message)
    } else {
        Ok(output.bytes)
    }
}
fn text(output: Output) -> Result<String, String> {
    decode(&checked(output)?).map(|value| value.trim_end_matches(['\r', '\n']).into())
}
fn valid_sha(sha: &str) -> bool {
    matches!(sha.len(), 40 | 64) && sha.bytes().all(|byte| byte.is_ascii_hexdigit())
}
fn hash(value: &str) -> String {
    let mut result = 0xcbf29ce484222325_u64;
    for byte in value.bytes() {
        result ^= byte as u64;
        result = result.wrapping_mul(0x100000001b3);
    }
    format!("local-{result:016x}")
}

pub(super) struct Repository {
    pub exe: PathBuf,
    pub root: PathBuf,
    pub id: String,
    pub head: Option<String>,
    pub branch: Option<String>,
    pub shallow: bool,
    pub deadline: Instant,
}
impl Repository {
    pub fn open(directory: &Path, seconds: u64) -> Result<Option<Self>, String> {
        let deadline = Instant::now() + Duration::from_secs(seconds);
        if !directory.is_dir() {
            return Err("项目目录暂时不可用".into());
        }
        let exe = runner::executable()?;
        let initial = runner::run(
            &exe,
            directory,
            &["rev-parse", "--is-inside-work-tree"],
            deadline,
            MAX_OUTPUT,
        )?;
        if !initial.success {
            if initial.timed_out || initial.truncated {
                return Err(warning(&initial).unwrap());
            }
            if directory.ancestors().any(|path| path.join(".git").exists()) {
                return Err("Git 仓库读取失败，请检查仓库权限与 Git safe.directory 配置".into());
            }
            return Ok(None);
        }
        if decode(&initial.bytes)?.trim() != "true" {
            return Err("项目不是可读取的 Git 工作区".into());
        }
        let root = PathBuf::from(text(runner::run(
            &exe,
            directory,
            &["rev-parse", "--show-toplevel"],
            deadline,
            MAX_OUTPUT,
        )?)?);
        let common = PathBuf::from(text(runner::run(
            &exe,
            &root,
            &["rev-parse", "--git-common-dir"],
            deadline,
            MAX_OUTPUT,
        )?)?);
        let common = if common.is_absolute() {
            common
        } else {
            root.join(common)
        };
        // 检查发生在 HEAD、log、diff 等对象读取之前，旧版 Git 也不会隐式下载缺失对象。
        let partial = runner::run(
            &exe,
            &root,
            &[
                "config",
                "--get-regexp",
                "^(extensions\\.partialclone|remote\\..*\\.promisor)$",
            ],
            deadline,
            MAX_OUTPUT,
        )?;
        if partial.timed_out || partial.truncated {
            return Err(warning(&partial).unwrap());
        }
        let partial_config = decode(&partial.bytes)?
            .lines()
            .any(|line| line.starts_with("extensions.") || !line.ends_with(" false"));
        let promisor = fs::read_dir(common.join("objects/pack"))
            .ok()
            .into_iter()
            .flatten()
            .filter_map(Result::ok)
            .any(|entry| {
                entry
                    .path()
                    .extension()
                    .is_some_and(|ext| ext == "promisor")
            });
        if partial_config || promisor {
            return Err("部分克隆暂不支持读取：当前实现无法保证所有 Git 版本禁用懒获取，请先在外部准备完整本地仓库".into());
        }
        let shallow = text(runner::run(
            &exe,
            &root,
            &["rev-parse", "--is-shallow-repository"],
            deadline,
            MAX_OUTPUT,
        )?)? == "true";
        let head_output = runner::run(
            &exe,
            &root,
            &["rev-parse", "--verify", "--quiet", "HEAD"],
            deadline,
            MAX_OUTPUT,
        )?;
        let head = if head_output.success {
            let sha = text(head_output)?;
            if !valid_sha(&sha) {
                return Err("Git HEAD 标识无效".into());
            }
            Some(sha)
        } else if head_output.timed_out || head_output.truncated {
            return Err(warning(&head_output).unwrap());
        } else {
            None
        };
        let branch_output = runner::run(
            &exe,
            &root,
            &["symbolic-ref", "--quiet", "--short", "HEAD"],
            deadline,
            MAX_OUTPUT,
        )?;
        let branch = if branch_output.success {
            Some(text(branch_output)?)
        } else if branch_output.timed_out || branch_output.truncated {
            return Err(warning(&branch_output).unwrap());
        } else {
            None
        };
        let canonical = common
            .canonicalize()
            .map_err(|_| "Git 仓库来源目录不可读取")?;
        let created = fs::metadata(&canonical)
            .ok()
            .and_then(|meta| meta.created().ok())
            .map(|time| format!("{time:?}"))
            .unwrap_or_default();
        let id = hash(&format!("{}:{created}", canonical.to_string_lossy()));
        Ok(Some(Self {
            exe,
            root,
            id,
            head,
            branch,
            shallow,
            deadline,
        }))
    }
    fn run(&self, args: &[&str], cap: usize) -> Result<Output, String> {
        runner::run(&self.exe, &self.root, args, self.deadline, cap)
    }
    fn require(directory: &Path, seconds: u64) -> Result<Self, String> {
        Self::open(directory, seconds)?.ok_or_else(|| "项目尚未初始化 Git 仓库".into())
    }
    fn verify(&self, sha: &str) -> Result<(), String> {
        if !valid_sha(sha) {
            return Err("提交 ID 必须是完整的十六进制 SHA".into());
        }
        if text(self.run(&["cat-file", "-t", sha], MAX_OUTPUT)?)? != "commit" {
            return Err("Git 对象不是提交".into());
        }
        Ok(())
    }
    fn parent(&self, sha: &str) -> Result<Option<String>, String> {
        self.verify(sha)?;
        // rev-list 会隐藏浅克隆边界的父项，必须读取提交对象原始头。
        let raw = checked(self.run(&["cat-file", "-p", sha], MAX_OUTPUT)?)?;
        for line in raw
            .split(|byte| *byte == b'\n')
            .take_while(|line| !line.is_empty())
        {
            if let Some(parent) = line.strip_prefix(b"parent ") {
                let parent = decode(parent)?;
                if !valid_sha(&parent) {
                    return Err("提交父对象标识无效".into());
                }
                // 本地没有父树时不能把浅边界当作真正根提交。
                checked(self.run(
                    &["cat-file", "-e", &format!("{parent}^{{tree}}")],
                    MAX_OUTPUT,
                )?)
                .map_err(|_| {
                    "父提交树在本地不可读，可能位于浅克隆边界；无法可靠比较此提交".to_string()
                })?;
                return Ok(Some(parent));
            }
        }
        Ok(None)
    }
    fn stable(&self) -> Result<(), String> {
        let current = self.run(&["rev-parse", "--verify", "--quiet", "HEAD"], MAX_OUTPUT)?;
        if current.timed_out || current.truncated {
            return Err(warning(&current).unwrap());
        }
        let current = if current.success {
            Some(text(current)?)
        } else {
            None
        };
        if current != self.head {
            Err("Git HEAD 已变化，请刷新后重新读取".into())
        } else {
            Ok(())
        }
    }
}

#[derive(Clone)]
struct CachedStatus {
    value: StatusSnapshot,
    created: Instant,
}
fn status_cache() -> &'static Mutex<HashMap<PathBuf, CachedStatus>> {
    static CACHE: OnceLock<Mutex<HashMap<PathBuf, CachedStatus>>> = OnceLock::new();
    CACHE.get_or_init(Mutex::default)
}
pub fn status_cached(directory: &Path, refresh: bool) -> Result<StatusSnapshot, String> {
    let repo = Repository::open(directory, 5)?;
    let key = directory.canonicalize().map_err(|_| "项目目录暂时不可用")?;
    if !refresh {
        if let Some(cached) = status_cache()
            .lock()
            .map_err(|_| "Git 状态缓存异常")?
            .get(&key)
        {
            if cached.created.elapsed() < Duration::from_secs(15)
                && repo.as_ref().is_some_and(|repo| {
                    cached.value.repository_id.as_ref() == Some(&repo.id)
                        && cached.value.head == repo.head
                        && cached.value.branch == repo.branch
                })
            {
                return Ok(cached.value.clone());
            }
        }
    }
    let value = status(directory)?;
    let mut cache = status_cache().lock().map_err(|_| "Git 状态缓存异常")?;
    cache.retain(|_, entry| entry.created.elapsed() < Duration::from_secs(15));
    if cache.len() >= 200 {
        cache.clear();
    }
    cache.insert(
        key,
        CachedStatus {
            value: value.clone(),
            created: Instant::now(),
        },
    );
    Ok(value)
}
pub fn status(directory: &Path) -> Result<StatusSnapshot, String> {
    let Some(repo) = Repository::open(directory, 5)? else {
        return Ok(StatusSnapshot {
            repository: false,
            repository_id: None,
            branch: None,
            head: None,
            read_at: now(),
            complete: true,
            warning: None,
            files: vec![],
            latest_commit_at: None,
            shallow: false,
        });
    };
    let output = repo.run(
        &[
            "status",
            "--porcelain=v2",
            "-z",
            "--untracked-files=all",
            "--ignore-submodules=dirty",
        ],
        MAX_OUTPUT,
    );
    let output = match output {
        Ok(output) => output,
        Err(problem) if problem.contains("过滤器") => Output {
            bytes: vec![],
            success: false,
            truncated: false,
            timed_out: false,
            stderr: problem,
        },
        Err(problem) => return Err(problem),
    };
    let mut problem = warning(&output);
    if repo.root.join(".gitmodules").exists() {
        problem =
            Some("子模块仅显示提交指针变化，内部工作区未扫描；请进入子模块项目单独查看。".into());
    }
    let mut files = Vec::new();
    let mut fields = output.bytes.split(|byte| *byte == 0);
    while let Some(record) = fields.next() {
        if record.is_empty() {
            continue;
        }
        if files.len() == MAX_STATUS {
            problem = Some("状态达到 5000 个文件上限，结果不完整".into());
            break;
        }
        let result: Result<FileStatus, String> = (|| {
            let tag = record[0];
            if tag == b'?' {
                return Ok(FileStatus {
                    path: decode(record.get(2..).ok_or("Git 状态记录不完整")?)?,
                    old_path: None,
                    index_status: "?".into(),
                    worktree_status: "?".into(),
                    untracked: true,
                    conflicted: false,
                });
            }
            let count = match tag {
                b'1' => 9,
                b'2' => 10,
                b'u' => 11,
                _ => return Err("Git 状态记录类型无法识别".into()),
            };
            let parts = record
                .splitn(count, |byte| *byte == b' ')
                .collect::<Vec<_>>();
            if parts.len() != count || parts[1].len() != 2 {
                return Err("Git 状态记录不完整".into());
            }
            let old_path = if tag == b'2' {
                Some(decode(fields.next().ok_or("Git 重命名记录不完整")?)?)
            } else {
                None
            };
            Ok(FileStatus {
                path: decode(parts[count - 1])?,
                old_path,
                index_status: (parts[1][0] as char).to_string(),
                worktree_status: (parts[1][1] as char).to_string(),
                untracked: false,
                conflicted: tag == b'u',
            })
        })();
        match result {
            Ok(file) => files.push(file),
            Err(error) => {
                problem = Some(error);
                break;
            }
        }
    }
    let latest_commit_at = if let Some(head) = &repo.head {
        Some(utc(&text(repo.run(
            &["log", "--max-count=1", "--format=%cI", head, "--"],
            MAX_OUTPUT,
        )?)?)?)
    } else {
        None
    };
    repo.stable()?;
    Ok(StatusSnapshot {
        repository: true,
        repository_id: Some(repo.id),
        branch: repo.branch,
        head: repo.head,
        read_at: now(),
        complete: problem.is_none(),
        warning: problem,
        files,
        latest_commit_at,
        shallow: repo.shallow,
    })
}

#[derive(Clone)]
struct Cursor {
    project: String,
    repository: String,
    root: PathBuf,
    branch: Option<String>,
    head: String,
    offset: usize,
    created: Instant,
}
fn cursors() -> &'static Mutex<HashMap<String, Cursor>> {
    static CURSORS: OnceLock<Mutex<HashMap<String, Cursor>>> = OnceLock::new();
    CURSORS.get_or_init(|| Mutex::new(HashMap::new()))
}
/// 备份导入或项目库替换时调用；游标属于应用派生内存数据。
pub fn invalidate_cache() {
    if let Ok(mut items) = status_cache().lock() {
        items.clear();
    }
    if let Ok(mut items) = history_cache().lock() {
        items.clear();
    }
    if let Ok(mut items) = cursors().lock() {
        items.clear();
    }
    if let Ok(mut items) = activity_cache().lock() {
        items.clear();
    }
}

#[derive(Clone)]
struct CachedHistory {
    value: History,
    created: Instant,
}
fn history_cache() -> &'static Mutex<HashMap<String, CachedHistory>> {
    static CACHE: OnceLock<Mutex<HashMap<String, CachedHistory>>> = OnceLock::new();
    CACHE.get_or_init(Mutex::default)
}
pub fn history(
    directory: &Path,
    project_id: &str,
    cursor: Option<&str>,
) -> Result<History, String> {
    let repo = Repository::require(directory, 10)?;
    let offset = if let Some(token) = cursor {
        let items = cursors().lock().map_err(|_| "Git 分页缓存不可用")?;
        let item = items.get(token).ok_or("Git 分页游标无效或已过期，请刷新")?;
        if item.created.elapsed() > Duration::from_secs(300)
            || item.project != project_id
            || item.repository != repo.id
            || item.root != repo.root
            || item.branch != repo.branch
            || Some(&item.head) != repo.head.as_ref()
        {
            return Err("Git 仓库或 HEAD 已变化，分页游标失效，请刷新".into());
        }
        item.offset
    } else {
        0
    };
    let cache_key = format!(
        "{}:{}:{:?}:{:?}:{offset}:{project_id}",
        repo.id,
        repo.root.display(),
        repo.head,
        repo.branch
    );
    if let Some(cached) = history_cache()
        .lock()
        .map_err(|_| "Git 历史缓存异常")?
        .get(&cache_key)
    {
        if cached.created.elapsed() < Duration::from_secs(240) {
            return Ok(cached.value.clone());
        }
    }
    let Some(head) = &repo.head else {
        return Ok(History {
            head: None,
            commits: vec![],
            next_cursor: None,
            complete: true,
            read_at: now(),
            warning: None,
        });
    };
    let skip = format!("--skip={offset}");
    let output = repo.run(
        &[
            "log",
            "-z",
            "--max-count=31",
            &skip,
            "--format=%H%x00%s%x00%B%x00%an%x00%aI%x00%cI",
            head,
            "--",
        ],
        MAX_OUTPUT,
    )?;
    let mut problem = warning(&output);
    let mut commits = Vec::new();
    let fields = output.bytes.split(|byte| *byte == 0).collect::<Vec<_>>();
    for row in fields.chunks(6) {
        if row.len() == 1 && row[0].is_empty() {
            break;
        }
        if row.len() < 6 {
            problem = Some("提交历史输出不完整".into());
            break;
        }
        let values = row
            .iter()
            .map(|field| decode(field))
            .collect::<Result<Vec<_>, _>>();
        match values {
            Ok(value) if valid_sha(&value[0]) => commits.push(Commit {
                sha: value[0].clone(),
                subject: value[1].clone(),
                message: value[2].trim_end_matches('\n').into(),
                author: value[3].clone(),
                author_time: utc(&value[4])?,
                commit_time: utc(&value[5])?,
            }),
            _ => {
                problem = Some("提交历史包含无法识别的数据".into());
                break;
            }
        }
    }
    let next_cursor = if commits.len() > 30 && problem.is_none() {
        commits.truncate(30);
        let token = uuid::Uuid::new_v4().to_string();
        let mut items = cursors().lock().map_err(|_| "Git 分页缓存不可用")?;
        items.retain(|_, item| item.created.elapsed() < Duration::from_secs(300));
        if items.len() >= 256 {
            if let Some(key) = items
                .iter()
                .min_by_key(|(_, item)| item.created)
                .map(|(key, _)| key.clone())
            {
                items.remove(&key);
            }
        }
        items.insert(
            token.clone(),
            Cursor {
                project: project_id.into(),
                repository: repo.id.clone(),
                root: repo.root.clone(),
                branch: repo.branch.clone(),
                head: head.clone(),
                offset: offset + 30,
                created: Instant::now(),
            },
        );
        Some(token)
    } else {
        commits.truncate(30);
        None
    };
    repo.stable()?;
    let value = History {
        head: repo.head,
        commits,
        next_cursor,
        complete: problem.is_none(),
        read_at: now(),
        warning: problem,
    };
    if value.complete {
        let mut cache = history_cache().lock().map_err(|_| "Git 历史缓存异常")?;
        cache.retain(|_, entry| entry.created.elapsed() < Duration::from_secs(240));
        if cache.len() >= 200 {
            cache.clear();
        }
        cache.insert(
            cache_key,
            CachedHistory {
                value: value.clone(),
                created: Instant::now(),
            },
        );
    }
    Ok(value)
}

fn file_args<'a>(mode: &'a str, sha: &'a str, parent: Option<&'a str>) -> Vec<&'a str> {
    let mut args = vec![
        "diff-tree",
        "--root",
        "--no-commit-id",
        "-r",
        "-M",
        "--no-ext-diff",
        "--no-textconv",
        mode,
        "-z",
    ];
    if let Some(parent) = parent {
        args.push(parent);
    }
    args.extend([sha, "--"]);
    args
}
fn read_files(
    repo: &Repository,
    sha: &str,
    parent: Option<&str>,
) -> Result<(Vec<CommitFile>, Option<String>), String> {
    let names = repo.run(&file_args("--name-status", sha, parent), MAX_OUTPUT)?;
    let mut problem = warning(&names);
    let mut files = Vec::new();
    let mut fields = names.bytes.split(|byte| *byte == 0);
    while let Some(kind) = fields.next() {
        if kind.is_empty() {
            continue;
        }
        if files.len() == MAX_FILES {
            problem = Some("提交文件达到 2000 项上限，结果不完整".into());
            break;
        }
        let result: Result<CommitFile, String> = (|| {
            let status = decode(kind)?;
            let path = decode(fields.next().ok_or("提交文件记录不完整")?)?;
            let (old_path, path) = if status.starts_with(['R', 'C']) {
                (
                    Some(path),
                    decode(fields.next().ok_or("提交重命名记录不完整")?)?,
                )
            } else {
                (None, path)
            };
            Ok(CommitFile {
                path,
                old_path,
                status: status.chars().take(1).collect(),
                additions: None,
                deletions: None,
                binary: false,
            })
        })();
        match result {
            Ok(file) => files.push(file),
            Err(error) => {
                problem = Some(error);
                break;
            }
        }
    }
    let counts = repo.run(&file_args("--numstat", sha, parent), MAX_OUTPUT)?;
    if let Some(message) = warning(&counts) {
        problem = Some(message);
    }
    let mut fields = counts.bytes.split(|byte| *byte == 0);
    while let Some(record) = fields.next() {
        if record.is_empty() {
            continue;
        }
        let row = record.splitn(3, |byte| *byte == b'\t').collect::<Vec<_>>();
        if row.len() != 3 {
            problem = Some("提交行数记录不完整".into());
            break;
        }
        let path = if row[2].is_empty() {
            let _old = fields.next();
            fields.next()
        } else {
            Some(row[2])
        };
        let Some(path) = path else {
            problem = Some("提交重命名行数不完整".into());
            break;
        };
        let path = decode(path)?;
        if let Some(file) = files.iter_mut().find(|file| file.path == path) {
            file.binary = row[0] == b"-" || row[1] == b"-";
            if !file.binary {
                file.additions = decode(row[0])?.parse().ok();
                file.deletions = decode(row[1])?.parse().ok();
            }
        }
    }
    Ok((files, problem))
}
pub fn commit_files(directory: &Path, sha: &str) -> Result<CommitFiles, String> {
    let repo = Repository::require(directory, 10)?;
    let first_parent = repo.parent(sha)?;
    let (files, problem) = read_files(&repo, sha, first_parent.as_deref())?;
    repo.stable()?;
    Ok(CommitFiles {
        head: repo.head,
        sha: sha.into(),
        first_parent,
        files,
        complete: problem.is_none(),
        warning: problem,
    })
}

fn valid_path(path: &str) -> Result<(), String> {
    if path.is_empty()
        || path.contains('\0')
        || path.contains('\\')
        || Path::new(path)
            .components()
            .any(|part| !matches!(part, Component::Normal(_)))
        || path.starts_with('/')
    {
        return Err("文件路径必须是仓库内的字面相对路径".into());
    }
    Ok(())
}
fn preview(repo: &Repository, path: &str) -> Result<Diff, String> {
    let state = status(&repo.root)?;
    if !state
        .files
        .iter()
        .any(|file| file.path == path && file.untracked)
    {
        return Err("只能预览当前未跟踪的文件".into());
    }
    let target = repo.root.join(path);
    let mut current = repo.root.clone();
    for part in Path::new(path).components() {
        current.push(part);
        let meta = fs::symlink_metadata(&current).map_err(|_| "文件已变化或不可读取")?;
        if meta.file_type().is_symlink() {
            return Err("未跟踪文件预览不读取符号链接".into());
        }
    }
    let canonical = target.canonicalize().map_err(|_| "文件不可读取")?;
    if !canonical.starts_with(repo.root.canonicalize().map_err(|_| "仓库目录不可读取")?) {
        return Err("文件位于仓库范围外".into());
    }
    let file = fs::File::open(&canonical).map_err(|_| "文件不可读取")?;
    if !file.metadata().map_err(|_| "文件信息不可读取")?.is_file() {
        return Err("只能预览普通文件".into());
    }
    let mut bytes = Vec::new();
    file.take((MAX_DIFF + 1) as u64)
        .read_to_end(&mut bytes)
        .map_err(|_| "文件预览读取失败")?;
    let truncated = bytes.len() > MAX_DIFF;
    bytes.truncate(MAX_DIFF);
    render_diff(bytes, truncated, None, false)
}
fn render_diff(
    bytes: Vec<u8>,
    mut truncated: bool,
    mut problem: Option<String>,
    patch: bool,
) -> Result<Diff, String> {
    let binary = bytes.contains(&0)
        || (patch
            && bytes.split(|byte| *byte == b'\n').any(|line| {
                (line.starts_with(b"Binary files ") && line.ends_with(b" differ"))
                    || line == b"GIT binary patch"
            }));
    if binary {
        return Ok(Diff {
            text: String::new(),
            binary: true,
            truncated,
            warning: problem,
        });
    }
    let mut value = String::from_utf8_lossy(&bytes).into_owned();
    if std::str::from_utf8(&bytes).is_err() {
        problem = Some("内容包含非 UTF-8 字节，已使用替代字符显示".into());
    }
    if let Some((index, _)) = value.match_indices('\n').nth(4999) {
        if index + 1 < value.len() {
            value.truncate(index + 1);
            truncated = true;
        }
    }
    if truncated {
        problem = Some("内容已截断（最多 256 KiB / 5000 行）".into());
    }
    Ok(Diff {
        text: value,
        binary: false,
        truncated,
        warning: problem,
    })
}
pub fn diff(directory: &Path, sha: Option<&str>, stage: &str, path: &str) -> Result<Diff, String> {
    valid_path(path)?;
    if !matches!(stage, "worktree" | "index" | "untracked") {
        return Err("Git 差异范围无效".into());
    }
    let repo = Repository::require(directory, 10)?;
    if sha.is_none() && stage == "untracked" {
        return preview(&repo, path);
    }
    let mut args;
    let parent;
    let old_path;
    if let Some(sha) = sha {
        parent = repo.parent(sha)?;
        let (files, _) = read_files(&repo, sha, parent.as_deref())?;
        let file = files
            .iter()
            .find(|file| file.path == path)
            .ok_or("文件不属于该提交的变更列表")?;
        old_path = file.old_path.clone();
        args = vec![
            "diff-tree",
            "--root",
            "--no-commit-id",
            "-r",
            "-M",
            "-p",
            "--no-ext-diff",
            "--no-textconv",
        ];
        if let Some(parent) = &parent {
            args.push(parent);
        }
        args.extend([sha, "--"]);
    } else {
        let state = status(&repo.root)?;
        if let Some(warning) = state
            .warning
            .as_ref()
            .filter(|warning| warning.contains("过滤器"))
        {
            return Err(warning.clone());
        }
        let file = state
            .files
            .iter()
            .find(|file| file.path == path && !file.untracked)
            .ok_or("文件不属于当前工作区变更列表")?;
        if file.conflicted {
            return Ok(Diff {
                text: String::new(),
                binary: false,
                truncated: false,
                warning: Some("该文件存在冲突，请在外部 Git 工具中检查".into()),
            });
        }
        old_path = file.old_path.clone();
        args = vec!["diff", "-M", "--no-ext-diff", "--no-textconv", "--no-color"];
        if stage == "index" {
            args.push("--cached");
        }
        args.push("--");
    }
    args.push(path);
    if let Some(old) = &old_path {
        args.push(old);
    }
    let output = repo.run(&args, MAX_DIFF)?;
    let problem = warning(&output);
    let truncated = output.truncated || output.timed_out;
    if !output.success && !truncated {
        return Err(problem.unwrap_or_else(|| "Git 差异读取失败".into()));
    }
    repo.stable()?;
    render_diff(output.bytes, truncated, problem, true)
}

#[derive(Clone)]
struct CachedActivity {
    value: Activity,
    created: Instant,
}
fn activity_cache() -> &'static Mutex<HashMap<String, CachedActivity>> {
    static CACHE: OnceLock<Mutex<HashMap<String, CachedActivity>>> = OnceLock::new();
    CACHE.get_or_init(|| Mutex::new(HashMap::new()))
}
pub fn activity(directory: &Path) -> Result<Activity, String> {
    let repo = Repository::require(directory, 15)?;
    let Some(head) = &repo.head else {
        return Ok(Activity {
            head: None,
            timestamps: vec![],
            complete: true,
            read_at: now(),
            shallow: repo.shallow,
            warning: None,
        });
    };
    let key = format!("{}:{}:{head}", repo.id, repo.root.display());
    if let Some(cached) = activity_cache()
        .lock()
        .map_err(|_| "活跃度缓存不可用")?
        .get(&key)
    {
        if cached.created.elapsed() < Duration::from_secs(300) {
            return Ok(cached.value.clone());
        }
    }
    // 不使用 --since：提交者时间可以非单调，必须先遍历 HEAD 可达历史再由界面按本地日期筛选。
    let output = repo.run(
        &["log", "--max-count=20001", "--format=%cI", head, "--"],
        MAX_OUTPUT,
    )?;
    let mut problem = warning(&output);
    let raw = decode(&output.bytes)?;
    let mut timestamps = Vec::new();
    for line in raw.lines() {
        if chrono::DateTime::parse_from_rfc3339(line).is_err() {
            problem = Some("活跃度包含不完整或异常提交时间".into());
            break;
        }
        if timestamps.len() == MAX_ACTIVITY {
            problem = Some("已读取 20000 个提交，活跃度统计不完整".into());
            break;
        }
        timestamps.push(utc(line)?);
    }
    repo.stable()?;
    if repo.shallow && problem.is_none() {
        problem = Some("浅克隆：统计仅包含本地可见历史".into());
    }
    let value = Activity {
        head: repo.head,
        timestamps,
        complete: problem.is_none(),
        read_at: now(),
        shallow: repo.shallow,
        warning: problem,
    };
    if value.complete {
        let mut cache = activity_cache().lock().map_err(|_| "活跃度缓存不可用")?;
        cache.retain(|_, entry| entry.created.elapsed() < Duration::from_secs(300));
        if cache.len() >= 100 {
            cache.clear();
        }
        cache.insert(
            key,
            CachedActivity {
                value: value.clone(),
                created: Instant::now(),
            },
        );
    }
    Ok(value)
}

/// 日志／任务关联验证复用同一离线、安全读取边界，不接受引用表达式。
pub fn verify_commit(directory: &Path, sha: &str) -> Result<(String, Commit), String> {
    let repo = Repository::require(directory, 10)?;
    repo.verify(sha)?;
    let raw = checked(repo.run(
        &[
            "show",
            "-s",
            "--format=%H%x00%s%x00%B%x00%an%x00%aI%x00%cI",
            sha,
            "--",
        ],
        MAX_OUTPUT,
    )?)?;
    let parts = raw
        .split(|byte| *byte == 0)
        .map(decode)
        .collect::<Result<Vec<_>, _>>()?;
    if parts.len() != 6 {
        return Err("提交快照读取失败".into());
    }
    Ok((
        repo.id,
        Commit {
            sha: parts[0].clone(),
            subject: parts[1].clone(),
            message: parts[2].trim_end().into(),
            author: parts[3].clone(),
            author_time: utc(&parts[4])?,
            commit_time: utc(&parts[5])?,
        },
    ))
}

#[cfg(test)]
mod tests;
