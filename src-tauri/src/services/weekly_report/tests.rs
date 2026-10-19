use super::protocol::*;
#[test]
fn weekly_report_response_distinguishes_empty_tools_and_incomplete_output() {
    use serde_json::json;
    let valid = json!({"choices":[{"finish_reason":"stop","message":{"content":"正文","tool_calls":[]}}]});
    assert_eq!(super::generation::response_content(&valid).unwrap(), "正文");
    for calls in [json!(null), json!([])] {
        let mut response = valid.clone();
        response["choices"][0]["message"]["tool_calls"] = calls;
        assert_eq!(super::generation::response_content(&response).unwrap(), "正文");
    }
    let mut missing = valid.clone();
    missing["choices"][0]["message"].as_object_mut().unwrap().remove("tool_calls");
    assert_eq!(super::generation::response_content(&missing).unwrap(), "正文");
    for (reason, calls, expected) in [
        ("length", json!([]), "finish_reason=length"),
        ("stop", json!([{"type":"function","function":{"name":"exec"}}]), "工具调用"),
        ("tool_calls", json!([]), "工具调用"),
        ("function_call", json!(null), "工具调用"),
        ("stop", json!({"invalid":true}), "工具调用"),
        ("content_filter", json!([]), "拦截"),
        ("insufficient_system_resource", json!([]), "未正常完成"),
        ("aborted", json!([]), "未正常完成"),
    ] {
        let mut response = valid.clone();
        response["choices"][0]["finish_reason"] = json!(reason);
        response["choices"][0]["message"]["tool_calls"] = calls;
        assert!(super::generation::response_content(&response).unwrap_err().contains(expected));
    }
    let mut length = valid.clone();
    length["choices"][0]["finish_reason"] = json!("length");
    length["usage"] = json!({"completion_tokens":6000});
    assert!(super::generation::response_content(&length).unwrap_err().contains("6000 token"));
    let mut only_thinking = valid.clone();
    only_thinking["choices"][0]["message"]["content"] = json!("");
    only_thinking["choices"][0]["message"]["reasoning_content"] = json!("private reasoning");
    let error = super::generation::response_content(&only_thinking).unwrap_err();
    assert!(error.contains("未返回周报正文"));
    assert!(!error.contains("private reasoning"));
    let mut legacy = valid.clone();
    legacy["choices"][0]["message"]["function_call"] = json!({"name":"exec"});
    assert!(super::generation::response_content(&legacy).unwrap_err().contains("工具调用"));
}
#[test]
fn weekly_report_rejects_duplicate_selection_and_keeps_friday() {
    let mut c = ReportContent::default();
    c.range = ReportRange {
        start_date: "2026-10-12".into(),
        end_date: "2026-10-16".into(),
        time_zone: "Asia/Shanghai".into(),
        start_utc: "2026-10-11T16:00:00Z".into(),
        end_exclusive_utc: "2026-10-16T16:00:00Z".into(),
    };
    assert!(c.validate(false).is_ok());
    c.selected_outcome_ids = vec!["x".into(), "x".into()];
    assert!(c.validate(false).is_err());
    assert!(c.range.contains("2026-10-16T15:59:59Z").unwrap());
    assert!(!c.range.contains("2026-10-16T16:00:00Z").unwrap());
}
fn range() -> ReportRange {
    ReportRange {
        start_date: "2026-10-12".into(),
        end_date: "2026-10-16".into(),
        time_zone: "Asia/Shanghai".into(),
        start_utc: "2026-10-11T16:00:00Z".into(),
        end_exclusive_utc: "2026-10-16T16:00:00Z".into(),
    }
}
#[test]
fn weekly_report_real_git_author_and_non_monotonic_time() {
    use std::process::Command;
    let d = tempfile::tempdir().unwrap();
    let path = d.path();
    let git = |args: &[&str], date: Option<&str>| {
        let mut c = Command::new("git");
        c.current_dir(path)
            .args([
                "-c",
                "user.name=本人",
                "-c",
                "user.email=Me@Example.com",
                "-c",
                "commit.gpgsign=false",
            ])
            .args(args);
        if let Some(date) = date {
            c.env("GIT_AUTHOR_DATE", date)
                .env("GIT_COMMITTER_DATE", date);
        }
        let o = c.output().unwrap();
        assert!(o.status.success(), "{}", String::from_utf8_lossy(&o.stderr));
        String::from_utf8_lossy(&o.stdout).trim().to_string()
    };
    git(&["init"], None);
    std::fs::write(path.join("代码.txt"), "first").unwrap();
    git(&["add", "."], None);
    git(
        &["commit", "-m", "周五开发"],
        Some("2026-10-16T23:59:59+08:00"),
    );
    git(
        &["commit", "--allow-empty", "-m", "周末工作"],
        Some("2026-10-17T00:00:00+08:00"),
    );
    git(
        &["commit", "--allow-empty", "-m", "历史时间乱序"],
        Some("2026-10-10T00:00:00+08:00"),
    );
    let (items, complete) = crate::services::git::weekly_report::commits(
        path,
        "p",
        "项目",
        &range(),
        &[AuthorRule {
            kind: "email".into(),
            value: "me@example.COM".into(),
        }],
        None,
        || Ok(()),
    )
    .unwrap();
    assert!(complete);
    assert_eq!(items.len(), 1);
    assert_eq!(items[0].title, "周五开发");
    assert!(items[0].files.contains(&"代码.txt".into()));
    git(&["mv", "代码.txt", "新代码.txt"], None);
    git(
        &["commit", "-m", "重命名文件"],
        Some("2026-10-15T12:00:00+08:00"),
    );
    let (renamed, _) = crate::services::git::weekly_report::commits(
        path,
        "p",
        "项目",
        &range(),
        &[AuthorRule {
            kind: "email".into(),
            value: "me@example.com".into(),
        }],
        None,
        || Ok(()),
    )
    .unwrap();
    let item = renamed.iter().find(|s| s.title == "重命名文件").unwrap();
    assert_eq!(item.files, vec!["新代码.txt"]);
    assert_eq!(item.renames, vec![("代码.txt".into(), "新代码.txt".into())]);
    assert!(!crate::services::git::weekly_report::matches(
        &[AuthorRule {
            kind: "name".into(),
            value: "本".into()
        }],
        "本人",
        "x"
    ));
}
#[test]
fn weekly_report_model_references_and_empty_hints() {
    let source = ReportSource {
        id: "s1".into(),
        project_ids: vec!["p".into()],
        ..Default::default()
    };
    let valid = r#"{"outcomes":[{"id":"o1","projectIds":["p"],"text":"调整接口","sourceIds":["s1"]}],"difficulties":[],"plans":[],"reflection":""}"#;
    assert_eq!(
        super::generation::parse(valid, &[source.clone()], &Default::default())
            .unwrap()
            .outcomes[0]
            .origin,
        "ai"
    );
    assert!(super::generation::parse(
        &valid.replace("s1", "invented"),
        &[source.clone()],
        &Default::default()
    )
    .is_err());
    assert!(super::generation::parse(
        &valid.replace("\"reflection\":\"\"", "\"reflection\":\"凭空感悟\""),
        &[source],
        &Default::default()
    )
    .is_err());
}
#[test]
fn weekly_report_rejects_unmerged_outcome_list() {
    let source = ReportSource {
        id: "s1".into(),
        project_ids: vec!["p".into()],
        ..Default::default()
    };
    let draft = |count: usize| {
        let outcomes: Vec<String> = (0..count)
            .map(|i| {
                format!(
                    r#"{{"id":"o{i}","projectIds":["p"],"text":"项目—工作{i}","sourceIds":["s1"]}}"#
                )
            })
            .collect();
        format!(
            r#"{{"outcomes":[{}],"difficulties":[],"plans":[],"reflection":""}}"#,
            outcomes.join(",")
        )
    };
    assert!(super::generation::parse(&draft(10), &[source.clone()], &Default::default()).is_ok());
    let error = super::generation::parse(&draft(11), &[source], &Default::default()).unwrap_err();
    assert!(error.contains("10 条"), "{error}");
}
#[test]
fn weekly_report_export_preserves_text_and_template() {
    let content = ReportContent {
        department: "信息技术部".into(),
        reporter: "测试人员".into(),
        range: range(),
        outcomes: vec![Outcome {
            id: "o".into(),
            project_ids: vec![],
            text: "=1+1 <接口>&测试 😀\n第二行".into(),
            source_ids: vec![],
            origin: "manual".into(),
        }],
        selected_outcome_ids: vec!["o".into()],
        reflection: "通过接口联调梳理异常处理。".repeat(20),
        ..Default::default()
    };
    let bytes = super::export::xlsx(&content).unwrap();
    assert!(bytes.starts_with(b"PK\x03\x04"));
    assert_eq!(&bytes[bytes.len() - 22..bytes.len() - 18], b"PK\x05\x06");
    let md = super::export::markdown(&content).unwrap();
    assert!(md.contains("&lt;接口&gt;&amp;测试"));
    if let Ok(dir) = std::env::var("PROJECTHUB_WEEKLY_VERIFY_DIR") {
        let dir = std::path::Path::new(&dir);
        std::fs::create_dir_all(dir).unwrap();
        std::fs::write(dir.join("weekly-report.xlsx"), bytes).unwrap();
        std::fs::write(dir.join("weekly-report.md"), md).unwrap();
    }
}
