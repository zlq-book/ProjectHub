use super::*;
use crate::models::ProjectInput;
use std::{
    io::{Read, Write},
    net::TcpListener,
    process::Command,
};
fn git(path: &Path, args: &[&str]) {
    let output = Command::new("git")
        .current_dir(path)
        .args([
            "-c",
            "user.name=测试作者",
            "-c",
            "user.email=private@example.invalid",
            "-c",
            "commit.gpgsign=false",
        ])
        .args(args)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
}
fn fixture() -> (tempfile::TempDir, Database, Store, String) {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("project");
    std::fs::create_dir(&path).unwrap();
    git(&path, &["init"]);
    std::fs::write(path.join("safe.txt"), "before\n").unwrap();
    std::fs::write(path.join(".env"), "secret=DO_NOT_SEND\n").unwrap();
    git(&path, &["add", "."]);
    git(&path, &["commit", "-m", "安全测试"]);
    std::fs::write(path.join("safe.txt"), "after\n").unwrap();
    std::fs::write(path.join(".env"), "secret=DO_NOT_SEND_CHANGED\n").unwrap();
    let db = Database::new(directory.path().join("test.sqlite")).unwrap();
    let p = db
        .create(ProjectInput {
            name: "摘要项目".into(),
            path: path.to_string_lossy().into(),
            description: "".into(),
            category: "other".into(),
            tags: vec![],
            tech_stack: vec![],
            favorite: false,
            notes: "PRIVATE_NOTES".into(),
            default_ide: "system".into(),
        })
        .unwrap();
    let store = Store::new(directory.path().join("ai.json"));
    (directory, db, store, p.id)
}
fn configure(store: &Store, url: String) {
    store
        .save(config::ConfigInput {
            base_url: url,
            model: "mock".into(),
            api_key: "".into(),
            clear_api_key: false,
        })
        .unwrap();
}
#[test]
fn preview_requires_confirmation_excludes_secrets_and_sends_only_reviewed_wire() {
    let (_directory, db, store, id) = fixture();
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    listener.set_nonblocking(true).unwrap();
    configure(
        &store,
        format!("http://{}/v1", listener.local_addr().unwrap()),
    );
    let preview = preview(
        &db,
        &store,
        id.clone(),
        None,
        "worktree".into(),
        vec!["safe.txt".into(), ".env".into()],
    )
    .unwrap();
    assert_eq!(preview.files, vec!["safe.txt"]);
    assert_eq!(preview.excluded, vec![".env"]);
    assert!(preview.bytes <= LIMIT);
    for private in [
        "DO_NOT_SEND",
        "PRIVATE_NOTES",
        "private@example.invalid",
        &db.get(&id).unwrap().input.path,
    ] {
        assert!(!preview.text.contains(private));
    }
    assert!(
        matches!(listener.accept(),Err(error) if error.kind()==std::io::ErrorKind::WouldBlock),
        "预览不能发送HTTP请求"
    );
    let server = std::thread::spawn(move || {
        let until = Instant::now() + Duration::from_secs(15);
        let mut stream = loop {
            match listener.accept() {
                Ok((stream, _)) => break stream,
                Err(error)
                    if error.kind() == std::io::ErrorKind::WouldBlock && Instant::now() < until =>
                {
                    std::thread::sleep(Duration::from_millis(5))
                }
                other => panic!("模型请求未到达: {other:?}"),
            }
        };
        stream
            .set_read_timeout(Some(Duration::from_secs(5)))
            .unwrap();
        let mut bytes = vec![];
        let mut buffer = [0; 4096];
        let mut size = None;
        loop {
            let count = stream.read(&mut buffer).unwrap();
            assert!(count > 0);
            bytes.extend_from_slice(&buffer[..count]);
            if let Some(end) = bytes.windows(4).position(|part| part == b"\r\n\r\n") {
                let headers = String::from_utf8_lossy(&bytes[..end]);
                let length = headers
                    .lines()
                    .find_map(|line| {
                        line.to_lowercase()
                            .strip_prefix("content-length:")
                            .and_then(|value| value.trim().parse::<usize>().ok())
                    })
                    .unwrap();
                size = Some((end + 4, length));
            }
            if let Some((start, length)) = size {
                if bytes.len() >= start + length {
                    break;
                }
            }
            assert!(bytes.len() < 128 * 1024);
        }
        let (start, length) = size.unwrap();
        let request: Value = serde_json::from_slice(&bytes[start..start + length]).unwrap();
        let content=json!({"sections":[{"title":"主要改动","text":"更新文本，请核对行为。","files":["safe.txt"]}]}).to_string();
        let body =
            json!({"choices":[{"finish_reason":"stop","message":{"content":content}}]}).to_string();
        write!(stream,"HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\nContent-Type: application/json\r\n\r\n{}",body.len(),body).unwrap();
        request
    });
    let result = generate(&db, &store, &preview.token).unwrap();
    assert!(result.contains("safe.txt"));
    assert!(result.contains("测试状态：未执行"));
    let request = server.join().unwrap();
    assert_eq!(serde_json::to_vec(&request).unwrap().len(), preview.bytes);
    assert!(generate(&db, &store, &preview.token).is_err());
}
#[test]
fn preview_cannot_send_changed_worktree_or_cancelled_token() {
    let (_directory, db, store, id) = fixture();
    configure(&store, "http://127.0.0.1:9/v1".into());
    let selected = vec!["safe.txt".into()];
    let first = preview(
        &db,
        &store,
        id.clone(),
        None,
        "worktree".into(),
        selected.clone(),
    )
    .unwrap();
    std::fs::write(
        Path::new(&db.get(&id).unwrap().input.path).join("safe.txt"),
        "changed after preview\n",
    )
    .unwrap();
    assert!(generate(&db, &store, &first.token)
        .unwrap_err()
        .contains("已变化"));
    let second = preview(&db, &store, id, None, "worktree".into(), selected).unwrap();
    cancel(&second.token).unwrap();
    assert!(generate(&db, &store, &second.token)
        .unwrap_err()
        .contains("取消"));
}
#[test]
fn sensitive_and_full_wire_budget() {
    for path in [
        ".env",
        "config/.env.local",
        "key.pem",
        "private/id_rsa",
        "credentials.json",
    ] {
        assert!(sensitive(path));
    }
    assert!(!sensitive("src/environment.ts"));
    assert!(wire("test", &json!({"diff":"x".repeat(LIMIT)})).is_err());
}
#[test]
fn rejects_invented_source_and_only_renders_plain_text() {
    let files = HashSet::from(["src/main.rs".into()]);
    assert!(render(
        r#"{"sections":[{"title":"改动","text":"声称","files":["secret"]}]}"#,
        &files,
        None
    )
    .is_err());
    let result=render(r#"{"sections":[{"title":"改动","text":"<script>ignored</script>","files":["src/main.rs"]}]}"#,&files,None).unwrap();
    assert!(result.contains("测试状态：未执行"));
    assert!(result.contains("src/main.rs"));
}

#[test]
fn renamed_environment_file_remains_excluded_from_summary() {
    let (_directory, db, store, id) = fixture();
    configure(&store, "http://127.0.0.1:9/v1".into());
    let project = db.get(&id).unwrap();
    git(
        Path::new(&project.input.path),
        &["mv", ".env", "configuration.txt"],
    );
    let error = preview(
        &db,
        &store,
        id,
        None,
        "index".into(),
        vec!["configuration.txt".into()],
    )
    .err()
    .unwrap();
    assert!(error.contains("敏感文件"));
    assert!(error.contains("configuration.txt"));
}
