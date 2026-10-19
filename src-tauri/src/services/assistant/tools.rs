use super::protocol::{Filters, TimeField};
use crate::models::Project;
use serde_json::{json, Value};
// 不将模型服务需要以外的本地路径、URL或常见凭据行附在项目资料中。
pub(super) fn safe_text(text: &str) -> String {
    text.lines()
        .map(|line| {
            let lower = line.to_lowercase();
            if line.contains(":\\")
                || line.contains(":/")
                || line.contains("\\\\")
                || line.split_whitespace().any(|word| word.starts_with('/'))
                || [
                    "api_key=",
                    "api_key:",
                    "apikey=",
                    "api-key:",
                    "authorization:",
                    "bearer ",
                    "password=",
                    "secret=",
                    "token=",
                    "api_key",
                    "\"apikey\"",
                    "\"password\"",
                    "\"secret\"",
                    "\"token\"",
                    "private key",
                    "git@",
                ]
                .iter()
                .any(|key| lower.contains(key))
            {
                "[已隐藏路径、链接或敏感配置]"
            } else {
                line
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
}
pub(super) fn metadata(project: &Project) -> Result<Value, String> {
    let mut limit = 4000;
    loop {
        let cut = |text: &str| safe_text(text).chars().take(limit).collect::<String>();
        let list = |items: &[String]| {
            items
                .iter()
                .take((limit / 20).max(1))
                .map(|s| cut(s))
                .collect::<Vec<_>>()
        };
        let value = json!({"id":project.id,"name":cut(&project.input.name),"description":cut(&project.input.description),"category":project.input.category,"techStack":list(&project.input.tech_stack),"tags":list(&project.input.tags),"favorite":project.input.favorite,"available":project.available,"createdAt":project.created_at,"updatedAt":project.updated_at,"lastOpenedAt":project.last_opened_at,"truncated":limit<4000});
        if value.to_string().len() <= 1200 {
            return Ok(value);
        }
        if limit == 1 {
            return Err("单个项目不可压缩的元数据过长，请修复项目标识或时间资料".into());
        }
        limit = (limit / 2).max(1);
    }
}
pub(super) fn clean_context(mut value: Value) -> Value {
    if let Some(files) = value["files"].as_array_mut() {
        for file in files {
            file["content"] = json!(safe_text(file["content"].as_str().unwrap_or_default()));
        }
    }
    value
}
pub(super) fn source(project: &Project, context: Option<&Value>, name: &str) -> Option<String> {
    match name {
        "name" => Some(safe_text(&project.input.name)),
        "description" => Some(safe_text(&project.input.description)),
        "category" => Some(project.input.category.clone()),
        "techStack" => Some(
            project
                .input
                .tech_stack
                .iter()
                .map(|s| safe_text(s))
                .collect::<Vec<_>>()
                .join("\n"),
        ),
        "tags" => Some(
            project
                .input
                .tags
                .iter()
                .map(|s| safe_text(s))
                .collect::<Vec<_>>()
                .join("\n"),
        ),
        "createdAt" => Some(project.created_at.clone()),
        "updatedAt" => Some(project.updated_at.clone()),
        "lastOpenedAt" => project.last_opened_at.clone(),
        _ => context
            .and_then(|c| c["files"].as_array())
            .and_then(|files| files.iter().find(|f| f["name"] == name))
            .and_then(|file| file["content"].as_str())
            .map(str::to_owned),
    }
}
pub(super) fn filtered(project: &Project, filters: &Filters) -> Result<bool, String> {
    if filters
        .category
        .as_ref()
        .is_some_and(|c| !crate::models::valid_category(c))
    {
        return Err("模型筛选分类无效".into());
    }
    if filters
        .category
        .as_ref()
        .is_some_and(|c| *c != project.input.category)
        || filters
            .favorite
            .is_some_and(|v| v != project.input.favorite)
        || filters.available.is_some_and(|v| v != project.available)
    {
        return Ok(false);
    }
    if filters.after.is_some() || filters.before.is_some() {
        let value = match filters.time_field {
            Some(TimeField::UpdatedAt) => Some(&project.updated_at),
            Some(TimeField::LastOpenedAt) => project.last_opened_at.as_ref(),
            None => return Err("时间条件缺少明确的资料更新或打开指标".into()),
        };
        let Some(value) = value else {
            return Ok(filters.include_never_opened
                && matches!(filters.time_field, Some(TimeField::LastOpenedAt)));
        };
        let value = chrono::DateTime::parse_from_rfc3339(value).map_err(|_| "项目时间数据无效")?;
        if let Some(after) = &filters.after {
            if value
                < chrono::DateTime::parse_from_rfc3339(after).map_err(|_| "模型开始时间无效")?
            {
                return Ok(false);
            }
        }
        if let Some(before) = &filters.before {
            if value
                > chrono::DateTime::parse_from_rfc3339(before).map_err(|_| "模型结束时间无效")?
            {
                return Ok(false);
            }
        }
    }
    Ok(true)
}
// 仅对完整的简单时间查询修正30天边界；外围否定、多个条件及复杂表达
// 保留模型已解析并经校验的边界，不能用其中的正向子串覆盖完整语义。
pub(super) fn monthly_time_filters(
    prompt: &str,
    reference_time: &str,
    filters: &mut Filters,
) -> Result<(), String> {
    let compact: String = prompt.chars().filter(|c| !c.is_whitespace()).collect();
    let mut expression = compact.as_str();
    loop {
        let next = [
            "请",
            "帮我找到",
            "帮我查找",
            "帮我找",
            "查找",
            "找到",
            "筛选",
            "列出",
            "找",
        ]
        .iter()
        .find_map(|prefix| expression.strip_prefix(prefix));
        match next {
            Some(rest) => expression = rest,
            None => break,
        }
    }
    loop {
        let next = ["的项目", "项目", "。", "？", "?"]
            .iter()
            .find_map(|suffix| expression.strip_suffix(suffix));
        match next {
            Some(rest) => expression = rest,
            None => break,
        }
    }
    let mut relation = None;
    for prefix in ["近一个月", "最近一个月", "近一个月内", "最近一个月内"] {
        for (field, negative, suffixes) in [
            (
                TimeField::LastOpenedAt,
                true,
                &[
                    "没有打开",
                    "没打开",
                    "未打开",
                    "都没有打开",
                    "都没打开",
                    "一直没打开",
                    "没有再打开",
                ][..],
            ),
            (
                TimeField::UpdatedAt,
                true,
                &[
                    "资料没有更新",
                    "资料没更新",
                    "资料未更新",
                    "没有更新资料",
                    "未更新资料",
                    "没有修改资料",
                    "资料没有修改",
                ][..],
            ),
            (
                TimeField::LastOpenedAt,
                false,
                &["打开", "打开过", "有打开"][..],
            ),
            (
                TimeField::UpdatedAt,
                false,
                &["资料更新", "更新资料", "修改资料", "资料修改"][..],
            ),
        ] {
            if suffixes
                .iter()
                .any(|suffix| expression == format!("{prefix}{suffix}"))
            {
                relation = Some((field, negative));
                break;
            }
        }
        if relation.is_some() {
            break;
        }
    }
    let Some((field, negative)) = relation else {
        return Ok(());
    };
    let reference =
        chrono::DateTime::parse_from_rfc3339(reference_time).map_err(|_| "本轮参考时间格式无效")?;
    let cutoff = reference - chrono::Duration::days(30);
    filters.include_never_opened = negative && matches!(field, TimeField::LastOpenedAt);
    filters.time_field = Some(field);
    if negative {
        filters.after = None;
        filters.before = Some(cutoff.to_rfc3339());
    } else {
        filters.after = Some(cutoff.to_rfc3339());
        filters.before = Some(reference.to_rfc3339());
    }
    Ok(())
}
pub(super) fn explicit_target(prompt: &str) -> Option<String> {
    for (category, labels) in [
        ("ai", vec!["AI 项目", "AI项目", "AI 类", "AI类"]),
        ("backend", vec!["后端"]),
        ("frontend", vec!["前端"]),
        ("tool", vec!["工具"]),
        ("learning", vec!["学习"]),
        ("other", vec!["其他"]),
    ] {
        for verb in ["归为", "归类到", "归到", "分类为", "分到"] {
            for label in &labels {
                if prompt.contains(&format!("{verb}{label}"))
                    || prompt.contains(&format!("{verb} {label}"))
                {
                    return Some(category.into());
                }
            }
        }
    }
    None
}
