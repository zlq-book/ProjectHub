use super::*;
fn input() -> SaveInput {
    let content = ReportContent {
        range: ReportRange {
            start_date: "2026-10-12".into(),
            end_date: "2026-10-16".into(),
            time_zone: "Asia/Shanghai".into(),
            start_utc: "2026-10-11T16:00:00Z".into(),
            end_exclusive_utc: "2026-10-16T16:00:00Z".into(),
        },
        ..Default::default()
    };
    SaveInput {
        report_id: None,
        version_id: None,
        parent_version_id: None,
        revision: None,
        content,
        sources: vec![],
        hints: Default::default(),
        context: Default::default(),
    }
}
#[test]
fn weekly_report_versions_and_backup_roundtrip() {
    let d = tempfile::tempdir().unwrap();
    let db = Database::new(d.path().join("a.sqlite")).unwrap();
    let old = db.weekly_save(input()).unwrap();
    let v = &old.versions[0];
    let mut next = input();
    next.report_id = Some(old.id.clone());
    next.parent_version_id = Some(v.id.clone());
    next.content.reflection = "人工补充".into();
    let new = db.weekly_save(next).unwrap();
    assert_eq!(new.versions.len(), 2);
    assert_eq!(new.adopted_version_id, v.id);
    let mut conflict = input();
    conflict.report_id = Some(old.id.clone());
    conflict.version_id = Some(v.id.clone());
    conflict.revision = Some(0);
    assert!(db.weekly_save(conflict).is_err());
    let file = d.path().join("backup.sqlite");
    db.export(&file).unwrap();
    let target = Database::new(d.path().join("b.sqlite")).unwrap();
    target.import(&file).unwrap();
    assert_eq!(target.weekly_get(&old.id).unwrap().versions.len(), 2);
}
#[test]
fn weekly_report_rejects_tampered_backup_and_preserves_current() {
    let dir = tempfile::tempdir().unwrap();
    let db = Database::new(dir.path().join("db.sqlite")).unwrap();
    let report = db.weekly_save(input()).unwrap();
    let bad = dir.path().join("bad.sqlite");
    db.export(&bad).unwrap();
    let c = Connection::open(&bad).unwrap();
    c.execute("UPDATE weekly_reports SET adopted_version_id='missing'", [])
        .unwrap();
    drop(c);
    assert!(db.import(&bad).is_err());
    assert_eq!(
        db.weekly_get(&report.id).unwrap().adopted_version_id,
        report.adopted_version_id
    );
    let legacy = dir.path().join("legacy.sqlite");
    db.export(&legacy).unwrap();
    let c = Connection::open(&legacy).unwrap();
    c.execute_batch("PRAGMA user_version=2;DROP TABLE weekly_report_sources;DROP TABLE weekly_report_versions;DROP TABLE weekly_reports;DROP TABLE weekly_report_preferences;").unwrap();
    drop(c);
    db.import(&legacy).unwrap();
    assert!(db.weekly_list().unwrap().is_empty());
    assert!(std::fs::read_dir(dir.path())
        .unwrap()
        .filter_map(Result::ok)
        .any(|e| e
            .file_name()
            .to_string_lossy()
            .starts_with("projecthub-before-import-")));
}
