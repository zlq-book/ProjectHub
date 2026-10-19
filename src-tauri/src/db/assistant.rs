use super::{error, Database, COLUMNS};
use crate::{
    models::{valid_category, Project},
    services::assistant::protocol::Change,
};
use rusqlite::{params, TransactionBehavior};
use std::collections::HashSet;
pub(crate) fn snapshot(project: &Project) -> serde_json::Value {
    let mut value = serde_json::to_value(project).expect("项目序列化");
    value.as_object_mut().unwrap().remove("available");
    value
}
impl Database {
    pub(crate) fn apply_assistant_patch(
        &self,
        field: &str,
        snapshots: &[Project],
        changes: &[Change],
    ) -> Result<Vec<Project>, String> {
        if !["description", "category"].contains(&field)
            || changes.is_empty()
            || changes.len() > 100
            || (field == "description" && changes.len() != 1)
        {
            return Err("助手修改字段或数量无效".into());
        }
        let mut ids = HashSet::new();
        let mut prepared = vec![];
        for change in changes {
            if !ids.insert(change.id.as_str()) {
                return Err("助手修改中存在重复项目".into());
            }
            let original = snapshots
                .iter()
                .find(|p| p.id == change.id)
                .ok_or("项目不属于当前助手建议")?;
            if (field == "description" && change.value.chars().count() > 4000)
                || (field == "category" && !valid_category(&change.value))
            {
                return Err("助手简介长度或分类值无效".into());
            }
            prepared.push((original, change));
        }
        let mut connection = self.connect()?;
        let tx = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(error)?;
        // 所有快照先校验再写入，任一资料变化使整批回滚。
        for (original, _) in &prepared {
            let current = tx
                .query_row(
                    &format!("SELECT {COLUMNS} FROM projects WHERE id=?1"),
                    [&original.id],
                    Self::row,
                )
                .map_err(|_| "建议中的项目已被删除，请刷新后重新生成")?;
            if snapshot(&current) != snapshot(original) {
                return Err("项目资料已变化，整批未保存，请重新生成建议".into());
            }
        }
        let now = chrono::Utc::now().to_rfc3339();
        let sql = if field == "description" {
            "UPDATE projects SET description=?2,updated_at=?3 WHERE id=?1"
        } else {
            "UPDATE projects SET category=?2,updated_at=?3 WHERE id=?1"
        };
        for (original, change) in &prepared {
            if (field == "category" && original.input.category == change.value)
                || (field == "description" && original.input.description == change.value)
            {
                continue;
            }
            tx.execute(sql, params![change.id, change.value, now])
                .map_err(error)?;
        }
        let mut result = vec![];
        for (original, _) in &prepared {
            result.push(
                tx.query_row(
                    &format!("SELECT {COLUMNS} FROM projects WHERE id=?1"),
                    [&original.id],
                    Self::row,
                )
                .map_err(error)?,
            );
        }
        tx.commit().map_err(error)?;
        Ok(result)
    }
}
