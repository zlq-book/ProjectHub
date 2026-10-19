use chrono::{DateTime, NaiveDate};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AuthorRule {
    pub kind: String,
    pub value: String,
}
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ReportRange {
    pub start_date: String,
    pub end_date: String,
    pub time_zone: String,
    pub start_utc: String,
    pub end_exclusive_utc: String,
}
impl ReportRange {
    pub fn validate(&self) -> Result<(), String> {
        let start =
            NaiveDate::parse_from_str(&self.start_date, "%Y-%m-%d").map_err(|_| "开始日期无效")?;
        let end =
            NaiveDate::parse_from_str(&self.end_date, "%Y-%m-%d").map_err(|_| "结束日期无效")?;
        if start > end || self.time_zone.is_empty() || self.time_zone.len() > 100 {
            return Err("日期范围或时区无效".into());
        }
        let a = DateTime::parse_from_rfc3339(&self.start_utc).map_err(|_| "起始时间无效")?;
        let b =
            DateTime::parse_from_rfc3339(&self.end_exclusive_utc).map_err(|_| "结束时间无效")?;
        let days = (end - start).num_days() + 1;
        if b <= a
            || ((b - a).num_seconds() - days * 86400).abs() > 7200
            || (a.date_naive() - start).num_days().abs() > 1
            || (b.date_naive() - end).num_days().abs() > 2
        {
            return Err("日期与 UTC 边界不一致".into());
        }
        Ok(())
    }
    pub fn contains(&self, value: &str) -> Result<bool, String> {
        let t = DateTime::parse_from_rfc3339(value).map_err(|_| "提交时间无效")?;
        Ok(
            t >= DateTime::parse_from_rfc3339(&self.start_utc).map_err(|_| "范围无效")?
                && t < DateTime::parse_from_rfc3339(&self.end_exclusive_utc)
                    .map_err(|_| "范围无效")?,
        )
    }
}
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ReportHints {
    pub difficulties: String,
    pub plans: String,
    pub reflection: String,
}
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Preferences {
    pub department: String,
    pub reporter: String,
    pub authors: Vec<AuthorRule>,
}
impl Preferences {
    pub fn validate(&self) -> Result<(), String> {
        if self.department.chars().count() > 200
            || self.reporter.chars().count() > 100
            || self.authors.len() > 50
            || self.authors.iter().any(|a| {
                !matches!(a.kind.as_str(), "email" | "name")
                    || a.value.trim().is_empty()
                    || a.value.len() > 320
                    || a.value.chars().any(char::is_control)
            })
        {
            Err("部门、填报人或作者规则无效".into())
        } else {
            Ok(())
        }
    }
}
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Outcome {
    pub id: String,
    pub project_ids: Vec<String>,
    pub text: String,
    pub source_ids: Vec<String>,
    #[serde(default)]
    pub origin: String,
}
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ReportContent {
    pub department: String,
    pub reporter: String,
    pub range: ReportRange,
    pub outcomes: Vec<Outcome>,
    pub selected_outcome_ids: Vec<String>,
    pub difficulties: Vec<String>,
    pub plans: Vec<String>,
    pub reflection: String,
}
impl ReportContent {
    pub fn validate(&self, exporting: bool) -> Result<(), String> {
        self.range.validate()?;
        if self.selected_outcome_ids.len() > 4 {
            return Err("主要完成工作最多选择 4 条".into());
        }
        if self.difficulties.len() > 3 || self.plans.len() > 4 {
            return Err("困难最多 3 条，下周计划最多 4 条".into());
        }
        if self.outcomes.len() > 100
            || self.reflection.chars().count() > 8000
            || self.department.chars().count() > 200
            || self.reporter.chars().count() > 100
        {
            return Err("周报内容超过容量限制".into());
        }
        let mut ids = HashSet::new();
        for o in &self.outcomes {
            if o.id.is_empty()
                || o.id.len() > 100
                || !ids.insert(&o.id)
                || !matches!(o.origin.as_str(), "ai" | "manual")
                || o.project_ids.len() > 20
                || o.source_ids.len() > 10200
                || o.text.chars().count() > 4000
            {
                return Err("成果格式或长度无效".into());
            }
        }
        let mut selected = HashSet::new();
        if self
            .selected_outcome_ids
            .iter()
            .any(|id| !ids.contains(id) || !selected.insert(id))
        {
            return Err("成果选择无效".into());
        }
        if self
            .difficulties
            .iter()
            .chain(&self.plans)
            .any(|s| s.chars().count() > 4000)
        {
            return Err("条目超过 4000 字符".into());
        }
        if exporting {
            if self.department.trim().is_empty() || self.reporter.trim().is_empty() {
                return Err("请填写部门和填报人".into());
            }
            if !self.selected_outcome_ids.iter().any(|id| {
                self.outcomes
                    .iter()
                    .any(|o| &o.id == id && !o.text.trim().is_empty())
            }) && !self
                .difficulties
                .iter()
                .chain(&self.plans)
                .any(|s| !s.trim().is_empty())
                && self.reflection.trim().is_empty()
            {
                return Err("请至少填写一项正文".into());
            }
        }
        Ok(())
    }
}
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ReportSource {
    pub id: String,
    pub kind: String,
    pub project_ids: Vec<String>,
    pub project_name: String,
    pub title: String,
    pub body: String,
    pub repository_id: Option<String>,
    pub sha: Option<String>,
    pub author: String,
    pub author_email: String,
    pub author_time: String,
    pub commit_time: String,
    pub log_id: Option<String>,
    #[serde(default)]
    pub log_date: String,
    #[serde(default)]
    pub log_commits: Vec<crate::db::development::CommitLink>,
    pub revision: Option<i64>,
    pub files: Vec<String>,
    #[serde(default)]
    pub renames: Vec<(String, String)>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProjectEvidence {
    pub project_id: String,
    pub project_name: String,
    pub path: String,
    pub repository_id: Option<String>,
    pub head: Option<String>,
    pub branch: Option<String>,
    pub complete: bool,
    pub warning: Option<String>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Collection {
    pub id: String,
    pub range: ReportRange,
    pub authors: Vec<AuthorRule>,
    pub projects: Vec<ProjectEvidence>,
    pub sources: Vec<ReportSource>,
    pub warnings: Vec<String>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ReportVersion {
    pub id: String,
    pub report_id: String,
    pub parent_version_id: Option<String>,
    pub revision: i64,
    pub content: ReportContent,
    pub sources: Vec<ReportSource>,
    pub hints: ReportHints,
    #[serde(default)]
    pub context: ReportContext,
    pub created_at: String,
    pub updated_at: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ReportGroup {
    pub id: String,
    pub adopted_version_id: String,
    pub versions: Vec<ReportVersion>,
    pub created_at: String,
}
#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SaveInput {
    pub report_id: Option<String>,
    pub version_id: Option<String>,
    pub parent_version_id: Option<String>,
    pub revision: Option<i64>,
    pub content: ReportContent,
    pub sources: Vec<ReportSource>,
    pub hints: ReportHints,
    #[serde(default)]
    pub context: ReportContext,
}
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ReportContext {
    pub authors: Vec<AuthorRule>,
    pub projects: Vec<ProjectEvidence>,
    pub diffs: Vec<DiffSelection>,
    pub model: String,
    pub base_url: String,
    pub warnings: Vec<String>,
}
#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CollectInput {
    pub project_ids: Vec<String>,
    pub range: ReportRange,
    pub authors: Vec<AuthorRule>,
    pub request_id: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DiffSelection {
    pub source_id: String,
    pub path: String,
}
#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PreviewInput {
    pub collection_id: String,
    pub source_ids: Vec<String>,
    pub diffs: Vec<DiffSelection>,
    pub hints: ReportHints,
}
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ReportPreview {
    pub token: String,
    pub model: String,
    pub base_url: String,
    pub text: String,
    pub bytes: usize,
    pub expires_at: String,
    pub warnings: Vec<String>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct GeneratedDraft {
    pub outcomes: Vec<Outcome>,
    pub difficulties: Vec<String>,
    pub plans: Vec<String>,
    pub reflection: String,
}
