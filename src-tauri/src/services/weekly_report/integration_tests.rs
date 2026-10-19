use super::{collection, generation, protocol::*};
use crate::{
    db::Database,
    models::ProjectInput,
    services::ai::{config::ConfigInput, Store},
};
use std::{
    io::{Read, Write},
    net::TcpListener,
    path::Path,
    process::Command,
    time::{Duration, Instant},
};
fn git(path: &Path, args: &[&str]) {
    let out = Command::new("git")
        .current_dir(path)
        .args([
            "-c",
            "user.name=本人",
            "-c",
            "user.email=me@example.com",
            "-c",
            "commit.gpgsign=false",
        ])
        .args(args)
        .env("GIT_AUTHOR_DATE", "2026-10-15T12:00:00+08:00")
        .env("GIT_COMMITTER_DATE", "2026-10-15T12:00:00+08:00")
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
}
#[test]
fn weekly_report_preview_exact_payload_and_single_use() {
    let dir = tempfile::tempdir().unwrap();
    let project = dir.path().join("项目");
    std::fs::create_dir(&project).unwrap();
    git(&project, &["init"]);
    std::fs::write(project.join("safe.txt"), "CODE_SHOULD_NOT_SEND_BY_DEFAULT").unwrap();
    std::fs::write(project.join(".env"), "SECRET_SHOULD_NEVER_SEND").unwrap();
    git(&project, &["add", "."]);
    git(&project, &["commit", "-m", "调整接口"]);
    let db = Database::new(dir.path().join("db.sqlite")).unwrap();
    let p = db
        .create(ProjectInput {
            name: "项目甲".into(),
            path: project.to_string_lossy().into(),
            description: String::new(),
            category: "other".into(),
            tags: vec![],
            tech_stack: vec![],
            favorite: false,
            notes: String::new(),
            default_ide: "system".into(),
        })
        .unwrap();
    let range = ReportRange {
        start_date: "2026-10-12".into(),
        end_date: "2026-10-16".into(),
        time_zone: "Asia/Shanghai".into(),
        start_utc: "2026-10-11T16:00:00Z".into(),
        end_exclusive_utc: "2026-10-16T16:00:00Z".into(),
    };
    let c = collection::collect(
        &db,
        CollectInput {
            project_ids: vec![p.id.clone()],
            range,
            authors: vec![AuthorRule {
                kind: "email".into(),
                value: "me@example.com".into(),
            }],
            request_id: uuid::Uuid::new_v4().to_string(),
        },
    )
    .unwrap();
    assert_eq!(c.sources.len(), 1);
    let server = TcpListener::bind("127.0.0.1:0").unwrap();
    server.set_nonblocking(true).unwrap();
    let store = Store::new(dir.path().join("ai.json"));
    store
        .save(ConfigInput {
            base_url: format!("http://{}/v1", server.local_addr().unwrap()),
            model: "mock".into(),
            api_key: String::new(),
            clear_api_key: false,
        })
        .unwrap();
    let input = || PreviewInput {
        collection_id: c.id.clone(),
        source_ids: vec![c.sources[0].id.clone()],
        diffs: vec![],
        hints: Default::default(),
    };
    let preview = generation::preview(&db, &store, input()).unwrap();
    assert!(server.accept().is_err());
    assert!(!preview.text.contains("CODE_SHOULD_NOT_SEND_BY_DEFAULT"));
    assert!(!preview.text.contains("SECRET_SHOULD_NEVER_SEND"));
    assert!(!preview
        .text
        .contains(&project.to_string_lossy().to_string()));
    let reviewed: serde_json::Value = serde_json::from_str(&preview.text).unwrap();
    let source_id = c.sources[0].id.clone();
    let pid = p.id.clone();
    let thread = std::thread::spawn(move || {
        let deadline = Instant::now() + Duration::from_secs(15);
        let mut stream = loop {
            match server.accept() {
                Ok((s, _)) => break s,
                Err(e)
                    if e.kind() == std::io::ErrorKind::WouldBlock && Instant::now() < deadline =>
                {
                    std::thread::sleep(Duration::from_millis(10))
                }
                Err(e) => panic!("模拟模型未收到请求：{e}"),
            }
        };
        stream
            .set_read_timeout(Some(Duration::from_secs(10)))
            .unwrap();
        let mut bytes = vec![];
        let mut buf = [0u8; 4096];
        let (header_end, length) = loop {
            let n = stream.read(&mut buf).unwrap();
            assert!(n > 0);
            bytes.extend_from_slice(&buf[..n]);
            if let Some(end) = bytes.windows(4).position(|w| w == b"\r\n\r\n") {
                let headers = String::from_utf8_lossy(&bytes[..end]);
                let length = headers
                    .lines()
                    .find_map(|line| {
                        line.to_lowercase()
                            .strip_prefix("content-length:")
                            .and_then(|v| v.trim().parse::<usize>().ok())
                    })
                    .unwrap();
                break (end + 4, length);
            }
        };
        while bytes.len() < header_end + length {
            let n = stream.read(&mut buf).unwrap();
            assert!(n > 0);
            bytes.extend_from_slice(&buf[..n]);
        }
        let actual: serde_json::Value =
            serde_json::from_slice(&bytes[header_end..header_end + length]).unwrap();
        assert_eq!(actual, reviewed);
        let content=serde_json::json!({"outcomes":[{"id":"o1","projectIds":[pid.clone()],"text":"项目甲—调整接口","sourceIds":[source_id.clone()]},{"id":"o2","projectIds":[pid],"text":"项目甲—补充查询与筛选能力","sourceIds":[source_id]}],"difficulties":[],"plans":[],"reflection":""}).to_string();
        let body =
            serde_json::json!({"choices":[{"finish_reason":"stop","message":{"content":content,"tool_calls":[]}}]})
                .to_string();
        write!(stream,"HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",body.len(),body).unwrap();
    });
    let result = generation::generate(&db, &store, &preview.token).unwrap();
    thread.join().unwrap();
    assert_eq!(result.outcomes.len(), 2);
    assert!(generation::generate(&db, &store, &preview.token).is_err());
    let cancelled = generation::preview(&db, &store, input()).unwrap();
    generation::cancel(&cancelled.token).unwrap();
    assert!(generation::generate(&db, &store, &cancelled.token).is_err());
    let changed = generation::preview(&db, &store, input()).unwrap();
    git(&project, &["commit", "--allow-empty", "-m", "改变 HEAD"]);
    assert!(generation::generate(&db, &store, &changed.token)
        .unwrap_err()
        .contains("HEAD"));
    let space = db.development_space(&p.id).unwrap();
    db.development_save_log(crate::db::development::LogInput {
        id: None,
        space_id: space.id,
        title: "本地日志".into(),
        date: "2026-10-15".into(),
        body: "无 Git 身份仍可汇报".into(),
        revision: None,
        commits: vec![],
    })
    .unwrap();
    let logs = collection::collect(
        &db,
        CollectInput {
            project_ids: vec![p.id],
            range: c.range.clone(),
            authors: vec![],
            request_id: uuid::Uuid::new_v4().to_string(),
        },
    )
    .unwrap();
    assert!(!logs.projects[0].complete);
    assert_eq!(logs.sources[0].log_date, "2026-10-15");
    let preview = generation::preview(
        &db,
        &store,
        PreviewInput {
            collection_id: logs.id.clone(),
            source_ids: vec![logs.sources[0].id.clone()],
            diffs: vec![],
            hints: Default::default(),
        },
    )
    .unwrap();
    assert!(preview.text.contains("无 Git 身份仍可汇报"));
    // 仅官方的新 DeepSeek 模型关闭默认思考，其他兼容接口保持通用参数。
    for (base_url, model, disabled) in [
        ("https://api.deepseek.com/v1", "deepseek-flash", true),
        ("https://api.deepseek.com", "deepseek-v4-pro", true),
        ("https://api.deepseek.com/v1", "custom-model", false),
        ("https://api.deepseek.com.example.org/v1", "deepseek-flash", false),
        ("http://127.0.0.1:8080/v1", "deepseek-flash", false),
    ] {
        store.save(ConfigInput { base_url: base_url.into(), model: model.into(), api_key: String::new(), clear_api_key: false }).unwrap();
        let preview = generation::preview(&db, &store, PreviewInput {
            collection_id: logs.id.clone(), source_ids: vec![logs.sources[0].id.clone()],
            diffs: vec![], hints: Default::default(),
        }).unwrap();
        let body: serde_json::Value = serde_json::from_str(&preview.text).unwrap();
        assert_eq!(body["thinking"]["type"].as_str(), disabled.then_some("disabled"));
        generation::cancel(&preview.token).unwrap();
    }
}
