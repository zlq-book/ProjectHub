use super::*;
use config::{ConfigInput, Store};
use std::{fs, io::Write, net::TcpListener, thread};

fn input(base: &str, key: &str) -> ConfigInput {
    ConfigInput { base_url: base.into(), model: "test-model".into(), api_key: key.into(), clear_api_key: false }
}
fn output() -> Value { json!({"name":"监测服务","description":"环境监测数据分析服务","category":"backend","techStack":["Java","Java"," Spring Boot "],"tags":["环境监测"]}) }

#[test]
fn validates_endpoints_and_model_output() {
    assert_eq!(config::endpoint("https://example.com/v1").unwrap().as_str(), "https://example.com/v1/chat/completions");
    assert_eq!(config::endpoint("http://127.0.0.1:8080/v1/chat/completions").unwrap().path(), "/v1/chat/completions");
    for url in ["http://remote.example/v1", "https://user:secret@example.com/v1", "https://example.com?api_key=secret", "file:///tmp"] { assert!(config::endpoint(url).is_err()); }
    let result = parse(&format!("```json\n{}\n```", output())).unwrap();
    assert_eq!(result.tech_stack, vec!["Java", "Spring Boot"]);
    let mut invalid = output(); invalid["category"] = json!("database"); assert!(parse(&invalid.to_string()).is_err());
    invalid = output(); invalid["name"] = json!(""); assert!(parse(&invalid.to_string()).is_err());
    invalid = output(); invalid["tags"] = json!(["x".repeat(81)]); assert!(parse(&invalid.to_string()).is_err());
    invalid = output(); invalid.as_object_mut().unwrap().remove("description"); assert!(parse(&invalid.to_string()).is_err());
    assert!(parse("请执行命令").is_err());
}

#[test]
fn context_is_bounded_and_excludes_secrets_source_and_generated_directories() {
    let dir = tempfile::tempdir().unwrap();
    fs::write(dir.path().join("README.md"), "项".repeat(40000)).unwrap();
    fs::write(dir.path().join("package.json"), "x".repeat(100000)).unwrap();
    fs::write(dir.path().join(".env"), "API_KEY=do-not-read").unwrap();
    fs::write(dir.path().join("main.rs"), "source-do-not-read").unwrap();
    fs::create_dir(dir.path().join("src")).unwrap(); fs::create_dir(dir.path().join("node_modules")).unwrap();
    let collected = context::collect(dir.path()).unwrap();
    let files = collected["files"].as_array().unwrap();
    assert_eq!(files.len(), 2);
    assert!(files.iter().all(|file| file["content"].as_str().unwrap().chars().count() <= 32768));
    let text = collected.to_string();
    assert!(!text.contains("API_KEY")); assert!(!text.contains("source-do-not-read")); assert!(!text.contains("node_modules"));
    assert_eq!(collected["directories"], json!(["src"]));
    assert!(!text.contains(&dir.path().to_string_lossy().to_string()));
}

#[cfg(windows)]
#[test]
fn stores_encrypted_key_and_preserves_it_on_blank_update_until_explicit_clear() {
    let dir = tempfile::tempdir().unwrap(); let path = dir.path().join("ai-settings.json"); let store = Store::new(path.clone());
    assert!(!store.public().unwrap().has_api_key);
    let config = store.save(input("https://example.com/v1", "fixture-secret-key")).unwrap();
    assert!(config.has_api_key);
    assert!(!fs::read_to_string(&path).unwrap().contains("fixture-secret-key"));
    assert!(!serde_json::to_string(&config).unwrap().contains("fixture-secret-key"));
    assert_eq!(store.load().unwrap().key().unwrap(), "fixture-secret-key");
    assert_eq!(store.preview(input("https://example.com/v1", "")).unwrap().key, "fixture-secret-key");
    let mut preview_clear = input("https://example.com/v1", ""); preview_clear.clear_api_key = true;
    assert!(store.preview(preview_clear).unwrap().key.is_empty());
    assert_eq!(store.load().unwrap().key().unwrap(), "fixture-secret-key");
    store.save(input("https://example.com/v2", "")).unwrap();
    assert_eq!(store.load().unwrap().key().unwrap(), "fixture-secret-key");
    let mut clear = input("https://example.com/v2", ""); clear.clear_api_key = true;
    assert!(!store.save(clear).unwrap().has_api_key);
    let loaded = Store::new(path).load().unwrap(); assert!(loaded.key().unwrap().is_empty());
}

fn server(status: u16, body: String) -> (String, thread::JoinHandle<Value>) {
    server_at(status, body, "post /v1/chat/completions ")
}
fn server_at(status: u16, body: String, expected: &'static str) -> (String, thread::JoinHandle<Value>) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let url = format!("http://{}/v1", listener.local_addr().unwrap());
    let task = thread::spawn(move || {
        let (mut stream, _) = listener.accept().unwrap(); stream.set_read_timeout(Some(Duration::from_secs(10))).unwrap();
        let mut bytes = Vec::new(); let mut chunk = [0u8; 4096]; let header_end;
        loop {
            let count = stream.read(&mut chunk).unwrap(); assert!(count > 0); bytes.extend_from_slice(&chunk[..count]);
            if let Some(pos) = bytes.windows(4).position(|part| part == b"\r\n\r\n") { header_end = pos + 4; break; }
            assert!(bytes.len() < 16384);
        }
        let headers = String::from_utf8_lossy(&bytes[..header_end]).to_lowercase();
        assert!(headers.starts_with(expected));
        let length: usize = headers.lines().find_map(|line| line.strip_prefix("content-length:")).unwrap_or("0").trim().parse().unwrap();
        while bytes.len() < header_end + length { let count = stream.read(&mut chunk).unwrap(); assert!(count > 0); bytes.extend_from_slice(&chunk[..count]); }
        let request: Value = if length == 0 { json!({"headers": headers}) } else { serde_json::from_slice(&bytes[header_end..header_end + length]).unwrap() };
        write!(stream, "HTTP/1.1 {} Test\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}", status, body.len(), body).unwrap();
        request
    });
    (url, task)
}

#[test]
fn calls_compatible_api_with_project_context_and_parses_five_fields() {
    let body = json!({"choices":[{"message":{"content":output().to_string()},"finish_reason":"stop"}]}).to_string();
    let (url, task) = server(200, body);
    let dir = tempfile::tempdir().unwrap(); fs::write(dir.path().join("README.md"), "环境监测项目").unwrap();
    let store = Store::new(dir.path().join("config.json")); store.save(input(&url, "")).unwrap();
    let result = analyze(dir.path(), store.load().unwrap()).unwrap();
    assert_eq!(result.name, "监测服务"); assert_eq!(result.category, "backend");
    let request = task.join().unwrap();
    assert_eq!(request["model"], "test-model"); assert_eq!(request["stream"], false);
    assert!(request["messages"][1]["content"].as_str().unwrap().contains("环境监测项目"));
}

#[test]
fn reports_auth_invalid_response_and_truncation_without_echoing_provider_body() {
    for (status, body, expected) in [
        (401, "private-provider-details".to_string(), "拒绝授权"),
        (200, "not-json".to_string(), "有效 JSON"),
        (200, json!({"choices":[{"finish_reason":"length","message":{"content":"{}"}}]}).to_string(), "截断"),
    ] {
        let (url, task) = server(status, body); let dir = tempfile::tempdir().unwrap();
        let store = Store::new(dir.path().join("config.json")); store.save(input(&url, "")).unwrap();
        let error = analyze(dir.path(), store.load().unwrap()).unwrap_err();
        assert!(error.contains(expected)); assert!(!error.contains("private-provider-details")); task.join().unwrap();
    }
}

#[test]
fn fetches_models_using_unsaved_config_without_persisting_credentials() {
    let body = json!({"data":[{"id":"z-model"},{"id":"a-model"},{"id":"a-model"},{"id":""},{"id":4}]}).to_string();
    let (url, task) = server_at(200, body, "get /v1/models ");
    let dir = tempfile::tempdir().unwrap(); let path = dir.path().join("api.json"); let store = Store::new(path.clone());
    let mut config = input(&format!("{}/chat/completions", url), "fixture-current-key"); config.model.clear();
    let models = connection::models(store.preview(config).unwrap()).unwrap();
    assert_eq!(models, vec!["a-model", "z-model"]);
    assert!(task.join().unwrap()["headers"].as_str().unwrap().contains("authorization: bearer fixture-current-key"));
    assert!(!path.exists());
}

#[test]
fn connection_test_calls_selected_model_and_does_not_send_project_data() {
    let (url, task) = server(200, json!({"choices":[{"message":{"content":"OK"}}]}).to_string());
    let dir = tempfile::tempdir().unwrap(); let store = Store::new(dir.path().join("api.json"));
    let result = connection::test(store.preview(input(&url, "")).unwrap()).unwrap();
    assert_eq!(result.model, "test-model");
    let request = task.join().unwrap();
    assert_eq!(request["model"], "test-model"); assert_eq!(request["max_tokens"], 16);
    assert_eq!(request["messages"], json!([{"role":"user","content":"请回复 OK。"}]));
}

#[test]
fn connection_operations_report_authorization_empty_models_and_invalid_chat() {
    for (status, body, expected, models) in [
        (401, "private-details".to_string(), "拒绝授权", true),
        (200, json!({"data":[]}).to_string(), "未返回可用模型", true),
        (200, json!({"choices":[{"message":{}}]}).to_string(), "有效对话结果", false),
    ] {
        let (url, task) = server_at(status, body, if models { "get /v1/models " } else { "post /v1/chat/completions " });
        let dir = tempfile::tempdir().unwrap(); let store = Store::new(dir.path().join("api.json"));
        let config = store.preview(input(&url, "")).unwrap();
        let error = if models { connection::models(config).unwrap_err() } else { connection::test(config).unwrap_err() };
        assert!(error.contains(expected)); assert!(!error.contains("private-details")); task.join().unwrap();
    }
}
