use super::*;
use crate::services::git;
impl Engine {
    pub(super) fn execute_git(
        &self,
        db: &Database,
        req: &Request,
        ticket: &state::Ticket,
        tool: &GitTool,
        name: &Option<String>,
        before: &Option<String>,
        after: &Option<String>,
    ) -> Result<ResultData, String> {
        if req.scope_ids.len() > 20 {
            return Ok(notice(
                "每轮 Git 查询最多 20 个项目，请选择当前项目、缩小筛选范围或分批查询。未静默截断。",
                req.scope_ids.len(),
            ));
        }
        let before = before
            .as_deref()
            .map(chrono::DateTime::parse_from_rfc3339)
            .transpose()
            .map_err(|_| "Git 时间上界无效")?;
        let after = after
            .as_deref()
            .map(chrono::DateTime::parse_from_rfc3339)
            .transpose()
            .map_err(|_| "Git 时间下界无效")?;
        let mut items = vec![];
        let mut messages = vec![];
        let mut read = 0;
        for id in &req.scope_ids {
            self.check(ticket)?;
            if ticket.started.elapsed() > std::time::Duration::from_secs(90) {
                messages.push("本轮达到 90 秒预算，其余项目未读取，不能视为无活动。".into());
                break;
            }
            let project = db.get(id)?;
            let normalize = |text: &str| text.to_lowercase().replace([' ', '-', '_'], "");
            if name.as_ref().is_some_and(|name| {
                !normalize(&format!(
                    "{} {} {} {}",
                    project.input.name,
                    project.input.description,
                    project.input.tech_stack.join(" "),
                    project.input.tags.join(" ")
                ))
                .contains(&normalize(name))
            }) {
                continue;
            }
            read += 1;
            let path = Path::new(&project.input.path);
            let status = match git::status(path) {
                Ok(status) => status,
                Err(error) => {
                    messages.push(format!(
                        "{}：读取失败，开发状态未知（{error}）",
                        project.input.name
                    ));
                    continue;
                }
            };
            if !status.repository {
                messages.push(format!(
                    "{}：未初始化 Git，不能判为长期未开发",
                    project.input.name
                ));
                continue;
            }
            let latest = status.latest_commit_at.as_deref();
            if let Some(latest) = latest {
                let time =
                    chrono::DateTime::parse_from_rfc3339(latest).map_err(|_| "Git 提交时间无效")?;
                if before.is_some_and(|limit| time > limit)
                    || after.is_some_and(|limit| time < limit)
                {
                    continue;
                }
            } else if before.is_some() || after.is_some() {
                messages.push(format!(
                    "{}：暂无提交，无法匹配提交时间范围",
                    project.input.name
                ));
                continue;
            }
            let mut evidence = vec![
                Evidence {
                    source: "git.currentBranch".into(),
                    quote: format!(
                        "{} · HEAD {}",
                        status.branch.as_deref().unwrap_or("分离 HEAD"),
                        status.head.as_deref().unwrap_or("暂无提交")
                    ),
                },
                Evidence {
                    source: "git.latestCommitAt".into(),
                    quote: latest.unwrap_or("暂无提交").into(),
                },
            ];
            match tool {
                GitTool::Status => {
                    evidence.push(Evidence {
                        source: "git.status".into(),
                        quote: format!(
                            "{}{} 个未提交文件；{}",
                            if status.complete { "" } else { "至少 " },
                            status.files.len(),
                            status
                                .files
                                .iter()
                                .take(30)
                                .map(|f| f.path.as_str())
                                .collect::<Vec<_>>()
                                .join("、")
                        ),
                    });
                }
                GitTool::History => {
                    let history = git::history(path, id, None)?;
                    for commit in history.commits.iter().take(5) {
                        evidence.push(Evidence {
                            source: "git.commit".into(),
                            quote: format!(
                                "{} · {} · {} · {}",
                                commit.sha, commit.commit_time, commit.author, commit.subject
                            ),
                        });
                    }
                }
                GitTool::Activity => {
                    let activity = git::activity(path)?;
                    let reference = chrono::DateTime::parse_from_rfc3339(&req.reference_time)
                        .map_err(|_| "参考时间无效")?;
                    let start = reference - chrono::Duration::days(30);
                    let count = activity
                        .timestamps
                        .iter()
                        .filter_map(|time| chrono::DateTime::parse_from_rfc3339(time).ok())
                        .filter(|time| *time >= start && *time <= reference)
                        .count();
                    evidence.push(Evidence {
                        source: "git.activity".into(),
                        quote: format!(
                            "近30天 {}{} 次提交，当前 HEAD、全部作者、含合并；{}",
                            if activity.complete { "" } else { "至少 " },
                            count,
                            activity.warning.as_deref().unwrap_or("读取完整")
                        ),
                    });
                }
                GitTool::Summary => {
                    evidence.push(Evidence{source:"git.summary".into(),quote:"打开开发进度，选择提交或未提交文件后点击 AI 总结，核对独立发送预览。当前未发送代码差异。".into()});
                }
            }
            if let Some(warning) = status.warning {
                evidence.push(Evidence {
                    source: "git.warning".into(),
                    quote: warning,
                });
            }
            items.push(Item {
                project,
                evidence,
                certainty: "confirmed".into(),
                description: None,
                category: None,
            });
        }
        self.check(ticket)?;
        Ok(ResultData{kind:"git".into(),text:format!("已读取 {read} 个项目。结果来自当前分支可达的本地 Git 历史、全部作者；最近开发时间指 HEAD 的提交者时间，未使用资料更新时间。{}",if messages.is_empty(){String::new()}else{format!("\n{}",messages.join("\n"))}),items,proposal_id:None,tool_steps:vec!["已校验本轮项目范围和 Git 只读工具".into(),"已读取共享 Git 服务的真实快照；未发送源码 Diff".into()],scope_count:req.scope_ids.len()})
    }
}
