use crate::models::Project;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Request {
    pub session_id: String,
    pub request_id: String,
    pub prompt: String,
    pub scope_ids: Vec<String>,
    pub reference_time: String,
    pub history: Vec<History>,
    pub context_ids: Vec<String>,
}
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct History {
    pub role: Role,
    pub content: String,
}
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Role {
    User,
    Assistant,
}
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Evidence {
    pub source: String,
    pub quote: String,
}
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum Certainty {
    Confirmed,
    Uncertain,
}
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Item {
    pub project: Project,
    pub evidence: Vec<Evidence>,
    pub certainty: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub category: Option<String>,
}
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ResultData {
    pub kind: String,
    pub text: String,
    pub items: Vec<Item>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub proposal_id: Option<String>,
    pub tool_steps: Vec<String>,
    pub scope_count: usize,
}
#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ApplyRequest {
    pub session_id: String,
    pub proposal_id: String,
    pub changes: Vec<Change>,
}
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct Change {
    pub id: String,
    pub value: String,
}
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(tag = "kind", rename_all = "camelCase", deny_unknown_fields)]
pub enum Intent {
    Git {
        tool: GitTool,
        #[serde(default)]
        name: Option<String>,
        #[serde(default)]
        before: Option<String>,
        #[serde(default)]
        after: Option<String>,
    },
    Search {
        #[serde(default)]
        filters: Filters,
    },
    Description,
    Categories {
        #[serde(rename = "targetCategory", default)]
        target_category: Option<String>,
        #[serde(default)]
        filters: Filters,
    },
    Notice {
        text: String,
    },
}
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum GitTool {
    Status,
    History,
    Activity,
    Summary,
}
#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Filters {
    pub category: Option<String>,
    pub favorite: Option<bool>,
    pub available: Option<bool>,
    pub time_field: Option<TimeField>,
    pub after: Option<String>,
    pub before: Option<String>,
    #[serde(default)]
    pub include_never_opened: bool,
}
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum TimeField {
    UpdatedAt,
    LastOpenedAt,
}
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Selection {
    pub items: Vec<Candidate>,
}
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Candidate {
    pub id: String,
    pub evidence: Vec<Evidence>,
    pub certainty: Certainty,
    pub description: Option<String>,
    pub category: Option<String>,
    #[serde(default)]
    pub needs_context: bool,
}
pub fn parse_intent(content: &str) -> Result<Intent, String> {
    let intent: Intent =
        serde_json::from_str(content).map_err(|_| "助手意图格式无效或含有未知操作、字段")?;
    if let Intent::Git {
        before,
        after,
        name,
        ..
    } = &intent
    {
        for time in [before, after].into_iter().flatten() {
            chrono::DateTime::parse_from_rfc3339(time).map_err(|_| "Git 时间边界格式无效")?;
        }
        if name.as_ref().is_some_and(|name| name.chars().count() > 200) {
            return Err("Git 项目查询词过长".into());
        }
        if before.as_ref().zip(after.as_ref()).is_some_and(|(b, a)| {
            chrono::DateTime::parse_from_rfc3339(a).ok()
                > chrono::DateTime::parse_from_rfc3339(b).ok()
        }) {
            return Err("Git 时间边界顺序无效".into());
        }
    }
    match &intent {
        Intent::Categories {
            target_category: Some(category),
            ..
        } if !crate::models::valid_category(category) => return Err("助手目标分类无效".into()),
        Intent::Notice { text } if text.chars().count() > 2000 => return Err("助手提示过长".into()),
        _ => {}
    }
    if let Intent::Search { filters } | Intent::Categories { filters, .. } = &intent {
        if filters
            .category
            .as_ref()
            .is_some_and(|value| !crate::models::valid_category(value))
        {
            return Err("助手筛选分类无效".into());
        }
        if (filters.after.is_some() || filters.before.is_some()) && filters.time_field.is_none() {
            return Err("时间筛选必须指定资料更新或打开指标".into());
        }
        if filters.include_never_opened
            && (!matches!(filters.time_field, Some(TimeField::LastOpenedAt))
                || filters.before.is_none()
                || filters.after.is_some())
        {
            return Err("包含从未打开的项目只支持明确的打开时间上界".into());
        }
        let parse = |value: &Option<String>| {
            value
                .as_ref()
                .map(|text| {
                    chrono::DateTime::parse_from_rfc3339(text)
                        .map_err(|_| "助手时间边界格式无效".to_string())
                })
                .transpose()
        };
        let after = parse(&filters.after)?;
        let before = parse(&filters.before)?;
        if after
            .zip(before)
            .is_some_and(|(after, before)| after > before)
        {
            return Err("助手时间范围开始时间不能晚于结束时间".into());
        }
    }
    Ok(intent)
}
