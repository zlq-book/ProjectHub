use super::*;
use crate::models::ProjectInput;
use crate::services::ai::config::{ConfigInput, Store};
use serde_json::{json, Value};
use std::{
    io::{Read, Write},
    net::TcpListener,
    sync::Arc,
    thread,
    time::Duration,
};

#[test]
fn assistant_accepts_supported_intent_and_rejects_arbitrary_tools() {
    assert!(protocol::parse_intent(r#"{"kind":"search"}"#).is_ok());
    assert!(protocol::parse_intent(r#"{"kind":"shell","command":"whoami"}"#).is_err());
    assert!(protocol::parse_intent(r#"{"kind":"search","sql":"DELETE FROM projects"}"#).is_err());
}
#[test]
fn assistant_does_not_substitute_metadata_when_model_misroutes_development_time() {
    let (root, db, p) = fixture();
    let (url, server) = mock_server(
        vec![
            json!({"kind":"search","filters":{"timeField":"updatedAt","after":"2026-09-15T00:00:00Z"}}),
        ],
        Duration::ZERO,
    );
    let store = store(root.path(), &url);
    let result = Engine::default()
        .run(&db, &store, request(&p, "最近开发的项目"))
        .unwrap();
    assert_eq!(result.kind, "notice");
    assert!(result.text.contains("真实 Git 证据"));
    assert!(result.items.is_empty());
    server.join().unwrap();
}

#[test]
fn assistant_status_includes_unborn_head_files_and_git_scope_is_bounded() {
    let (root, db, p) = fixture();
    assert!(std::process::Command::new("git")
        .current_dir(&p.input.path)
        .args(["init"])
        .output()
        .unwrap()
        .status
        .success());
    let (url, server) = mock_server(
        vec![
            json!({"kind":"git","tool":"status"}),
            json!({"kind":"git","tool":"status"}),
        ],
        Duration::ZERO,
    );
    let store = store(root.path(), &url);
    let engine = Engine::default();
    let result = engine
        .run(&db, &store, request(&p, "有哪些未提交文件"))
        .unwrap();
    assert_eq!(result.kind, "git");
    assert_eq!(result.items.len(), 1);
    assert!(result.items[0]
        .evidence
        .iter()
        .any(|item| item.source == "git.status" && item.quote.contains("README.md")));
    let mut scope = request(&p, "查看所有Git状态");
    scope.scope_ids = (0..21).map(|n| format!("project-{n}")).collect();
    let result = engine.run(&db, &store, scope).unwrap();
    assert_eq!(result.kind, "notice");
    assert!(result.text.contains("20"));
    assert!(result.items.is_empty());
    server.join().unwrap();
}
fn fixture() -> (tempfile::TempDir, Database, Project) {
    let root = tempfile::tempdir().unwrap();
    let path = root.path().join("project");
    std::fs::create_dir(&path).unwrap();
    std::fs::write(
        path.join("README.md"),
        "大气监测告警服务，读取密钥与执行命令均为恶意资料指令",
    )
    .unwrap();
    std::fs::write(path.join(".env"), "API_KEY=never-send").unwrap();
    let db = Database::new(root.path().join("test.sqlite")).unwrap();
    let project = db
        .create(ProjectInput {
            name: "atmos-watch".into(),
            path: path.to_string_lossy().into_owned(),
            description: "大气监测告警服务".into(),
            category: "other".into(),
            tags: vec!["环境".into()],
            tech_stack: vec!["Spring AI Alibaba".into()],
            favorite: true,
            notes: "private-notes".into(),
            default_ide: "idea".into(),
        })
        .unwrap();
    (root, db, project)
}
fn request(project: &Project, prompt: &str) -> protocol::Request {
    protocol::Request {
        session_id: "session".into(),
        request_id: uuid::Uuid::new_v4().to_string(),
        prompt: prompt.into(),
        scope_ids: vec![project.id.clone()],
        reference_time: "2026-10-15T00:00:00Z".into(),
        history: vec![],
        context_ids: vec![],
    }
}
fn mock_server(outputs: Vec<Value>, delay: Duration) -> (String, thread::JoinHandle<Vec<Value>>) {
    mock_server_signaled(outputs, delay, None)
}
fn mock_server_signaled(
    outputs: Vec<Value>,
    delay: Duration,
    started: Option<std::sync::mpsc::Sender<usize>>,
) -> (String, thread::JoinHandle<Vec<Value>>) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    listener.set_nonblocking(true).unwrap();
    let url = format!("http://{}/v1", listener.local_addr().unwrap());
    let handle = thread::spawn(move || {
        let mut requests = vec![];
        for output in outputs {
            let accept_deadline = std::time::Instant::now() + Duration::from_secs(5);
            let (mut stream, _) = loop {
                match listener.accept() {
                    Ok(stream) => break stream,
                    Err(error)
                        if error.kind() == std::io::ErrorKind::WouldBlock
                            && std::time::Instant::now() < accept_deadline =>
                    {
                        thread::sleep(Duration::from_millis(5))
                    }
                    Err(_) => return requests,
                }
            };
            stream.set_nonblocking(false).unwrap();
            stream
                .set_read_timeout(Some(Duration::from_secs(10)))
                .unwrap();
            let mut bytes = vec![];
            let mut buffer = [0u8; 4096];
            let end;
            loop {
                let n = stream.read(&mut buffer).unwrap();
                assert!(n > 0);
                bytes.extend_from_slice(&buffer[..n]);
                if let Some(pos) = bytes.windows(4).position(|b| b == b"\r\n\r\n") {
                    end = pos + 4;
                    break;
                }
            }
            let headers = String::from_utf8_lossy(&bytes[..end]).to_lowercase();
            let len: usize = headers
                .lines()
                .find_map(|s| s.strip_prefix("content-length:"))
                .unwrap()
                .trim()
                .parse()
                .unwrap();
            while bytes.len() < end + len {
                let n = stream.read(&mut buffer).unwrap();
                assert!(n > 0);
                bytes.extend_from_slice(&buffer[..n]);
            }
            requests.push(serde_json::from_slice(&bytes[end..end + len]).unwrap());
            if let Some(started) = &started {
                let _ = started.send(requests.len());
            }
            thread::sleep(delay);
            let body=json!({"choices":[{"message":{"content":output.to_string()},"finish_reason":"stop"}]}).to_string();
            write!(stream,"HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",body.len(),body).unwrap();
        }
        requests
    });
    (url, handle)
}
fn store(root: &std::path::Path, url: &str) -> Store {
    let store = Store::new(root.join("config.json"));
    store
        .save(ConfigInput {
            base_url: url.into(),
            model: "mock-model".into(),
            api_key: "".into(),
            clear_api_key: false,
        })
        .unwrap();
    store
}
fn candidate(p: &Project) -> Value {
    json!({"id":p.id,"evidence":[{"source":"description","quote":"大气监测告警"}],"certainty":"confirmed"})
}
fn apply(
    engine: &Engine,
    db: &Database,
    id: String,
    changes: Vec<protocol::Change>,
) -> Result<Vec<Project>, String> {
    engine.apply(
        db,
        protocol::ApplyRequest {
            session_id: "session".into(),
            proposal_id: id,
            changes,
        },
    )
}

#[test]
fn assistant_semantic_search_uses_real_metadata_without_reading_files_or_private_fields() {
    let (root, db, p) = fixture();
    let (url, server) = mock_server(
        vec![json!({"kind":"search"}), json!({"items":[candidate(&p)]})],
        Duration::ZERO,
    );
    let result = Engine::default()
        .run(
            &db,
            &store(root.path(), &url),
            request(&p, "找到空气质量预警项目"),
        )
        .unwrap();
    assert_eq!(result.kind, "search");
    assert_eq!(result.items[0].project.id, p.id);
    assert_eq!(result.items[0].certainty, "confirmed");
    let requests = server.join().unwrap();
    let text = serde_json::to_string(&requests).unwrap();
    for private in ["private-notes", "never-send", "恶意资料指令", &p.input.path] {
        assert!(!text.contains(private), "泄漏：{private}");
    }
    assert!(text.contains("大气监测告警服务"));
}
#[test]
fn assistant_rejects_forged_id_and_forged_evidence() {
    for forged in [
        json!({"id":"forged","evidence":[],"certainty":"confirmed"}),
        json!({"id":"REAL","evidence":[{"source":"description","quote":"fabricated"}],"certainty":"confirmed"}),
    ] {
        let (root, db, p) = fixture();
        let mut forged = forged;
        if forged["id"] == "REAL" {
            forged["id"] = json!(p.id);
        }
        let (url, server) = mock_server(
            vec![json!({"kind":"search"}), json!({"items":[forged]})],
            Duration::ZERO,
        );
        assert!(Engine::default()
            .run(&db, &store(root.path(), &url), request(&p, "查找项目"))
            .is_err());
        server.join().unwrap();
    }
}
#[test]
fn assistant_description_requires_explicit_context_and_only_changes_description() {
    let (root, db, p) = fixture();
    let (url, server) = mock_server(
        vec![
            json!({"kind":"description"}),
            json!({"items":[candidate(&p)]}),
        ],
        Duration::ZERO,
    );
    let engine = Engine::default();
    let result = engine
        .run(&db, &store(root.path(), &url), request(&p, "生成简介"))
        .unwrap();
    assert_eq!(result.kind, "readContext");
    assert!(result.proposal_id.is_none());
    assert!(!serde_json::to_string(&server.join().unwrap())
        .unwrap()
        .contains("恶意资料指令"));
    let mut item = candidate(&p);
    item["description"] = json!("中文项目新简介");
    item["evidence"] = json!([{"source":"README.md","quote":"大气监测告警服务"}]);
    let (url, server) = mock_server(
        vec![json!({"kind":"description"}), json!({"items":[item]})],
        Duration::ZERO,
    );
    let mut req = request(&p, "生成简介");
    req.context_ids = vec![p.id.clone()];
    let result = engine.run(&db, &store(root.path(), &url), req).unwrap();
    let proposal = result.proposal_id.unwrap();
    let changes = vec![protocol::Change {
        id: p.id.clone(),
        value: "用户编辑的简介".into(),
    }];
    let saved = apply(&engine, &db, proposal.clone(), changes.clone()).unwrap();
    assert_eq!(saved[0].input.description, "用户编辑的简介");
    let mut original = serde_json::to_value(&p).unwrap();
    let mut actual = serde_json::to_value(&saved[0]).unwrap();
    for key in ["description", "updatedAt"] {
        original.as_object_mut().unwrap().remove(key);
        actual.as_object_mut().unwrap().remove(key);
    }
    assert_eq!(original, actual);
    let mut edited = saved[0].input.clone();
    edited.description = "后续人工修改".into();
    db.update(&p.id, edited).unwrap();
    assert_eq!(
        apply(&engine, &db, proposal, changes).unwrap()[0]
            .input
            .description,
        "用户编辑的简介"
    );
    assert_eq!(db.get(&p.id).unwrap().input.description, "后续人工修改");
    let text = serde_json::to_string(&server.join().unwrap()).unwrap();
    assert!(text.contains("恶意资料指令"));
    assert!(!text.contains("never-send"));
}
#[test]
fn assistant_categories_roll_back_conflicts_and_keep_explicit_target() {
    let (root, db, p) = fixture();
    let second_dir = root.path().join("second");
    std::fs::create_dir(&second_dir).unwrap();
    let mut input = p.input.clone();
    input.path = second_dir.to_string_lossy().into_owned();
    let q = db.create(input).unwrap();
    let mut a = candidate(&p);
    a["category"] = json!("backend");
    let mut b = candidate(&q);
    b["category"] = json!("tool");
    let (url, server) = mock_server(
        vec![
            json!({"kind":"categories","targetCategory":"ai"}),
            json!({"items":[a,b]}),
        ],
        Duration::ZERO,
    );
    let engine = Engine::default();
    let mut req = request(&p, "把 Spring AI Alibaba 项目归为 AI 项目");
    req.scope_ids.push(q.id.clone());
    let result = engine.run(&db, &store(root.path(), &url), req).unwrap();
    assert!(result
        .items
        .iter()
        .all(|i| i.category.as_deref() == Some("ai")));
    let mut input = q.input.clone();
    input.description = "冲突资料".into();
    db.update(&q.id, input).unwrap();
    assert!(apply(
        &engine,
        &db,
        result.proposal_id.unwrap(),
        vec![
            protocol::Change {
                id: p.id.clone(),
                value: "ai".into()
            },
            protocol::Change {
                id: q.id.clone(),
                value: "tool".into()
            }
        ]
    )
    .is_err());
    assert_eq!(db.get(&p.id).unwrap().input.category, "other");
    server.join().unwrap();
}
#[test]
fn assistant_cancel_and_invalidate_prevent_late_proposals() {
    for invalidate in [false, true] {
        let (root, db, p) = fixture();
        let mut item = candidate(&p);
        item["category"] = json!("ai");
        let (signal, receiver) = std::sync::mpsc::channel();
        let (url, server) = mock_server_signaled(
            vec![json!({"kind":"categories"}), json!({"items":[item]})],
            Duration::from_millis(150),
            Some(signal),
        );
        let store = Arc::new(store(root.path(), &url));
        let engine = Engine::default();
        let req = request(&p, "整理分类");
        let request_id = req.request_id.clone();
        let runner = engine.clone();
        let db_run = db.clone();
        let worker = thread::spawn(move || runner.run(&db_run, &store, req));
        assert_eq!(receiver.recv_timeout(Duration::from_secs(5)).unwrap(), 1);
        assert_eq!(receiver.recv_timeout(Duration::from_secs(5)).unwrap(), 2);
        if invalidate {
            engine.mutate(|| Ok(())).unwrap();
        } else {
            engine.cancel("session", &request_id).unwrap();
        }
        assert!(worker.join().unwrap().is_err());
        server.join().unwrap();
        assert!(engine.inner.lock().unwrap().proposals.is_empty());
    }
}
#[test]
fn assistant_rejects_scope_limits_context_escalation_and_reads_real_git_status() {
    let (root, db, p) = fixture();
    let store = Store::new(root.path().join("config.json"));
    let engine = Engine::default();
    let mut req = request(&p, "查找");
    req.scope_ids = vec!["x".into(); 101];
    assert!(engine.run(&db, &store, req).unwrap_err().contains("100"));
    let mut req = request(&p, "生成简介");
    req.context_ids = vec!["outside".into()];
    assert!(engine.run(&db, &store, req).is_err());
    let (url, server) = mock_server(vec![json!({"kind":"git","tool":"status"})], Duration::ZERO);
    store
        .save(ConfigInput {
            base_url: url,
            model: "mock-model".into(),
            api_key: "".into(),
            clear_api_key: false,
        })
        .unwrap();
    let result = engine
        .run(&db, &store, request(&p, "最近开发的项目"))
        .unwrap();
    assert_eq!(result.kind, "git");
    assert!(result.text.contains("未初始化 Git"));
    assert!(result.items.is_empty());
    server.join().unwrap();
}

#[test]
fn assistant_category_rows_apply_distinct_values_and_configuration_or_import_invalidates() {
    for mode in ["apply", "config", "import"] {
        let (root, db, p) = fixture();
        let second = root.path().join("second");
        std::fs::create_dir(&second).unwrap();
        let mut input = p.input.clone();
        input.path = second.to_string_lossy().into_owned();
        let q = db.create(input).unwrap();
        let mut a = candidate(&p);
        a["category"] = json!("ai");
        let mut b = candidate(&q);
        b["category"] = json!("backend");
        let (url, server) = mock_server(
            vec![json!({"kind":"categories"}), json!({"items":[a,b]})],
            Duration::ZERO,
        );
        let store = store(root.path(), &url);
        let engine = Engine::default();
        let mut req = request(&p, "整理项目分类");
        req.scope_ids.push(q.id.clone());
        let result = engine.run(&db, &store, req).unwrap();
        if mode == "config" {
            engine
                .mutate(|| {
                    store.save(ConfigInput {
                        base_url: url,
                        model: "changed-model".into(),
                        api_key: "".into(),
                        clear_api_key: false,
                    })
                })
                .unwrap();
        }
        if mode == "import" {
            let backup = root.path().join("backup.sqlite");
            db.export(&backup).unwrap();
            engine.mutate(|| db.import(&backup)).unwrap();
        }
        let result = apply(
            &engine,
            &db,
            result.proposal_id.unwrap(),
            vec![
                protocol::Change {
                    id: p.id.clone(),
                    value: "ai".into(),
                },
                protocol::Change {
                    id: q.id.clone(),
                    value: "backend".into(),
                },
            ],
        );
        if mode == "apply" {
            assert_eq!(result.unwrap().len(), 2);
            assert_eq!(db.get(&p.id).unwrap().input.category, "ai");
            assert_eq!(db.get(&q.id).unwrap().input.category, "backend");
        } else {
            assert!(result.is_err());
            assert_eq!(db.get(&p.id).unwrap().input.category, "other");
        }
        server.join().unwrap();
    }
}
#[test]
fn assistant_context_reports_missing_and_truncated_files() {
    let root = tempfile::tempdir().unwrap();
    std::fs::write(root.path().join("README.md"), "x".repeat(40000)).unwrap();
    let context = crate::services::ai::context::collect(root.path()).unwrap();
    assert_eq!(context["files"][0]["truncated"], true);
    assert!(context["warnings"].to_string().contains("截断"));
    assert!(
        crate::services::ai::context::collect(&root.path().join("missing"))
            .unwrap_err()
            .contains("不存在")
    );
    let root = tempfile::tempdir().unwrap();
    let context = crate::services::ai::context::collect(root.path()).unwrap();
    assert!(context["warnings"].to_string().contains("资料不足"));
}
#[test]
fn assistant_hundred_projects_are_semantically_checked_in_two_bounded_batches() {
    let (root, db, p) = fixture();
    let mut ids = vec![p.id.clone()];
    for index in 1..100 {
        let path = root.path().join(format!("p{index}"));
        std::fs::create_dir(&path).unwrap();
        let mut input = p.input.clone();
        input.path = path.to_string_lossy().into_owned();
        input.description = "很长的项目资料".repeat(500);
        ids.push(db.create(input).unwrap().id);
    }
    let (url, server) = mock_server(
        vec![
            json!({"kind":"search"}),
            json!({"items":[]}),
            json!({"items":[]}),
        ],
        Duration::ZERO,
    );
    let mut req = request(&p, "查找空气质量预警");
    req.scope_ids = ids;
    let result = Engine::default()
        .run(&db, &store(root.path(), &url), req)
        .unwrap();
    assert_eq!(result.scope_count, 100);
    let requests = server.join().unwrap();
    assert_eq!(requests.len(), 3);
    for request in &requests[1..] {
        let body: Value =
            serde_json::from_str(request["messages"][1]["content"].as_str().unwrap()).unwrap();
        assert_eq!(body["projects"].as_array().unwrap().len(), 50);
        assert!(serde_json::to_vec(&body["projects"]).unwrap().len() <= 64 * 1024);
        assert!(body["projects"]
            .as_array()
            .unwrap()
            .iter()
            .any(|p| p["truncated"] == true));
    }
}

#[test]
fn assistant_wire_budget_counts_system_prompt_history_and_context_and_rejects_long_ids() {
    let (root, db, p) = fixture();
    std::fs::write(
        std::path::Path::new(&p.input.path).join("README.md"),
        format!("大气监测告警服务\n{}", "长资料内容".repeat(6000)),
    )
    .unwrap();
    std::fs::write(
        std::path::Path::new(&p.input.path).join("pom.xml"),
        "额外依赖资料".repeat(6000),
    )
    .unwrap();
    let mut item = candidate(&p);
    item["description"] = json!("项目简介");
    let (url, server) = mock_server(
        vec![json!({"kind":"description"}), json!({"items":[item]})],
        Duration::ZERO,
    );
    let mut req = request(&p, "生成简介");
    req.context_ids = vec![p.id.clone()];
    let result = Engine::default()
        .run(&db, &store(root.path(), &url), req)
        .unwrap();
    let requests = server.join().unwrap();
    for request in requests {
        assert!(
            serde_json::to_vec(&request).unwrap().len() <= 64 * 1024,
            "实际模型请求必须包含system和上下文包装后仍不超过64KiB"
        );
    }
    assert!(result.text.contains("64 KiB"));
    let mut req = request(&p, "查找");
    req.scope_ids = vec!["x".repeat(10000)];
    let store = Store::new(root.path().join("unused.json"));
    assert!(Engine::default()
        .run(&db, &store, req)
        .unwrap_err()
        .contains("ID"));
    let mut malformed = p.clone();
    malformed.id = "x".repeat(10000);
    assert!(tools::metadata(&malformed).is_err());
}

#[test]
fn assistant_network_wait_does_not_hold_database_lock() {
    let (root, db, p) = fixture();
    let (url, server) = mock_server(
        vec![json!({"kind":"search"}), json!({"items":[candidate(&p)]})],
        Duration::from_millis(250),
    );
    let config = store(root.path(), &url);
    let engine = Engine::default();
    let req = request(&p, "查找项目");
    let runner = db.clone();
    let worker = thread::spawn(move || engine.run(&runner, &config, req));
    thread::sleep(Duration::from_millis(80));
    let started = std::time::Instant::now();
    let mut input = p.input.clone();
    input.favorite = false;
    db.update(&p.id, input).unwrap();
    assert!(
        started.elapsed() < Duration::from_millis(180),
        "模型等待期间不应占用数据库锁"
    );
    assert!(worker.join().unwrap().is_ok());
    server.join().unwrap();
}
#[test]
fn assistant_reference_time_and_exact_filters_are_deterministic() {
    let (root, db, p) = fixture();
    let connection = rusqlite::Connection::open(&db.path).unwrap();
    connection
        .execute(
            "UPDATE projects SET updated_at='2026-10-14T01:00:00Z' WHERE id=?1",
            [&p.id],
        )
        .unwrap();
    drop(connection);
    let (url, server) = mock_server(
        vec![
            json!({"kind":"search","filters":{"timeField":"lastOpenedAt","after":"2000-01-01T00:00:00Z"}}),
            json!({"items":[candidate(&p)]}),
        ],
        Duration::ZERO,
    );
    let result = Engine::default()
        .run(
            &db,
            &store(root.path(), &url),
            request(&p, "近一个月资料更新的项目"),
        )
        .unwrap();
    assert_eq!(result.items.len(), 1);
    let requests = server.join().unwrap();
    let payload: Value =
        serde_json::from_str(requests[1]["messages"][1]["content"].as_str().unwrap()).unwrap();
    assert_eq!(payload["intent"]["filters"]["timeField"], "updatedAt");
    assert_eq!(
        payload["intent"]["filters"]["after"],
        "2026-09-15T00:00:00+00:00"
    );
    assert_eq!(
        payload["intent"]["filters"]["before"],
        "2026-10-15T00:00:00+00:00"
    );
    assert!(protocol::parse_intent(r#"{"kind":"search","filters":{"shell":"whoami"}}"#).is_err());
}

#[test]
fn assistant_mixed_categories_keep_confirmed_proposal_and_exclude_unread_rows() {
    let (root, db, p) = fixture();
    let second = root.path().join("second");
    std::fs::create_dir(&second).unwrap();
    let mut input = p.input.clone();
    input.path = second.to_string_lossy().into_owned();
    let q = db.create(input).unwrap();
    let mut a = candidate(&p);
    a["category"] = json!("ai");
    let mut b = candidate(&q);
    b["category"] = json!("ai");
    b["needsContext"] = json!(true);
    let (url, server) = mock_server(
        vec![json!({"kind":"categories"}), json!({"items":[a,b]})],
        Duration::ZERO,
    );
    let engine = Engine::default();
    let mut req = request(&p, "整理分类");
    req.scope_ids.push(q.id.clone());
    let result = engine.run(&db, &store(root.path(), &url), req).unwrap();
    assert_eq!(result.kind, "categories");
    assert_eq!(result.items[0].category.as_deref(), Some("ai"));
    assert_eq!(result.items[1].certainty, "uncertain");
    assert!(result.items[1].category.is_none());
    assert!(result.text.contains("尚未完成"));
    let proposal = result.proposal_id.unwrap();
    assert!(apply(
        &engine,
        &db,
        proposal.clone(),
        vec![protocol::Change {
            id: q.id.clone(),
            value: "ai".into()
        }]
    )
    .is_err());
    apply(
        &engine,
        &db,
        proposal,
        vec![protocol::Change {
            id: p.id.clone(),
            value: "ai".into(),
        }],
    )
    .unwrap();
    assert_eq!(db.get(&q.id).unwrap().input.category, "other");
    server.join().unwrap();
}

#[test]
fn assistant_negative_monthly_open_filter_keeps_older_and_never_opened_projects() {
    let (root, db, p) = fixture();
    let mut others = vec![];
    for name in ["recent", "never"] {
        let path = root.path().join(name);
        std::fs::create_dir(&path).unwrap();
        let mut input = p.input.clone();
        input.path = path.to_string_lossy().into_owned();
        others.push(db.create(input).unwrap());
    }
    let connection = rusqlite::Connection::open(&db.path).unwrap();
    connection
        .execute(
            "UPDATE projects SET last_opened_at='2026-09-01T00:00:00Z' WHERE id=?1",
            [&p.id],
        )
        .unwrap();
    connection
        .execute(
            "UPDATE projects SET last_opened_at='2026-10-14T00:00:00Z' WHERE id=?1",
            [&others[0].id],
        )
        .unwrap();
    drop(connection);
    let (url, server) = mock_server(
        vec![
            json!({"kind":"search","filters":{"timeField":"lastOpenedAt","before":"2026-09-15T00:00:00Z"}}),
            json!({"items":[candidate(&p),candidate(&others[1])]}),
        ],
        Duration::ZERO,
    );
    let mut req = request(&p, "找近一个月没有打开的项目");
    req.scope_ids.extend(others.iter().map(|p| p.id.clone()));
    let result = Engine::default()
        .run(&db, &store(root.path(), &url), req)
        .unwrap();
    assert_eq!(result.items.len(), 2);
    assert!(result.items.iter().all(|p| p.project.id != others[0].id));
    assert!(result.text.contains("从未打开"));
    let requests = server.join().unwrap();
    let payload: Value =
        serde_json::from_str(requests[1]["messages"][1]["content"].as_str().unwrap()).unwrap();
    assert!(payload["intent"]["filters"]["after"].is_null());
    assert_eq!(
        payload["intent"]["filters"]["before"],
        "2026-09-15T00:00:00+00:00"
    );
}
#[test]
fn assistant_context_reads_readme_once_and_preserves_dependency_budget() {
    let root = tempfile::tempdir().unwrap();
    std::fs::write(root.path().join("README.md"), "x".repeat(32 * 1024)).unwrap();
    std::fs::write(root.path().join("pom.xml"), "Spring AI Alibaba").unwrap();
    let context = crate::services::ai::context::collect(root.path()).unwrap();
    let files = context["files"].as_array().unwrap();
    assert_eq!(
        files
            .iter()
            .filter(|f| f["name"]
                .as_str()
                .unwrap()
                .eq_ignore_ascii_case("readme.md"))
            .count(),
        1
    );
    assert!(
        files.iter().any(|f| f["name"] == "pom.xml"),
        "同一README不得重复消耗预算导致依赖清单未读取"
    );
}

#[test]
fn assistant_monthly_open_filter_does_not_negate_unrelated_description_condition() {
    let (_root, _db, mut project) = fixture();
    // 多条件请求保留模型正确解析的打开范围，不用其他条件中的否定重写它。
    let mut filters = protocol::Filters {
        time_field: Some(protocol::TimeField::LastOpenedAt),
        after: Some("2026-09-15T00:00:00Z".into()),
        before: Some("2026-10-15T00:00:00Z".into()),
        ..Default::default()
    };
    let original = serde_json::to_value(&filters).unwrap();
    tools::monthly_time_filters(
        "找近一个月打开但没有简介的项目",
        "2026-10-15T00:00:00Z",
        &mut filters,
    )
    .unwrap();
    assert_eq!(serde_json::to_value(&filters).unwrap(), original);
    assert!(!filters.include_never_opened);
    project.last_opened_at = Some("2026-10-14T00:00:00Z".into());
    assert!(tools::filtered(&project, &filters).unwrap());
    project.last_opened_at = Some("2026-09-01T00:00:00Z".into());
    assert!(!tools::filtered(&project, &filters).unwrap());
    project.last_opened_at = None;
    assert!(!tools::filtered(&project, &filters).unwrap());
}

#[test]
fn assistant_monthly_filter_preserves_exclusion_and_complex_model_boundaries() {
    for prompt in ["排除近一个月打开的项目", "找近一个月打开之前创建的项目"]
    {
        let mut filters = protocol::Filters {
            time_field: Some(protocol::TimeField::LastOpenedAt),
            before: Some("2026-09-15T00:00:00Z".into()),
            include_never_opened: true,
            ..Default::default()
        };
        let original = serde_json::to_value(&filters).unwrap();
        tools::monthly_time_filters(prompt, "2026-10-15T00:00:00Z", &mut filters).unwrap();
        assert_eq!(
            serde_json::to_value(&filters).unwrap(),
            original,
            "复杂时间关系不能被正向子串覆盖：{prompt}"
        );
    }
}

#[test]
fn assistant_negative_monthly_metadata_filter_uses_metadata_time() {
    let (_root, _db, mut project) = fixture();
    let mut filters = protocol::Filters::default();
    tools::monthly_time_filters(
        "找近一个月没有修改资料的项目",
        "2026-10-15T00:00:00Z",
        &mut filters,
    )
    .unwrap();
    assert!(!filters.include_never_opened);
    project.updated_at = "2026-10-14T00:00:00Z".into();
    assert!(!tools::filtered(&project, &filters).unwrap());
    project.updated_at = "2026-09-01T00:00:00Z".into();
    assert!(tools::filtered(&project, &filters).unwrap());
    assert!(protocol::parse_intent(r#"{"kind":"search","filters":{"timeField":"updatedAt","before":"2026-09-15T00:00:00Z","includeNeverOpened":true}}"#).is_err());
}

#[test]
fn assistant_context_preserves_distinct_case_sensitive_readmes() {
    let root = tempfile::tempdir().unwrap();
    std::fs::write(root.path().join("README.md"), "Upper README").unwrap();
    // 文件系统若不区分大小写，去重行为已由前一项用例覆盖。
    if root.path().join("readme.md").exists() {
        return;
    }
    std::fs::write(root.path().join("readme.md"), "Lower README").unwrap();
    let context = crate::services::ai::context::collect(root.path()).unwrap();
    let files = context["files"].as_array().unwrap();
    assert_eq!(files.len(), 2);
    assert!(files.iter().any(|file| file["content"] == "Upper README"));
    assert!(files.iter().any(|file| file["content"] == "Lower README"));
}
