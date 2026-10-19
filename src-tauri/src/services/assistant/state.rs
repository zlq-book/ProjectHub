use super::{
    protocol::{ApplyRequest, Change, Request},
    *,
};
use std::{
    collections::{HashMap, HashSet},
    sync::{Arc, Mutex, MutexGuard},
    time::{Duration, Instant},
};
#[derive(Clone, Default)]
pub struct Engine {
    pub(super) inner: Arc<Mutex<State>>,
}
#[derive(Default)]
pub(super) struct State {
    generation: u64,
    sessions: HashMap<String, Session>,
    pub(super) proposals: HashMap<String, Proposal>,
}
#[derive(Default)]
struct Session {
    epoch: u64,
    active: Option<String>,
    used: HashSet<String>,
}
pub(super) struct Proposal {
    session: String,
    request: String,
    generation: u64,
    epoch: u64,
    created: Instant,
    field: String,
    snapshots: Vec<Project>,
    applied: Option<(Vec<Change>, Vec<Project>)>,
}
pub struct Ticket {
    pub(super) started: Instant,
    session: String,
    request: String,
    generation: u64,
    epoch: u64,
}
impl Engine {
    fn lock(&self) -> Result<MutexGuard<'_, State>, String> {
        self.inner
            .lock()
            .map_err(|_| "助手状态锁异常，请重启应用".into())
    }
    pub(super) fn begin(&self, input: &Request) -> Result<Ticket, String> {
        let mut state = self.lock()?;
        state
            .proposals
            .retain(|_, p| p.created.elapsed() < Duration::from_secs(600));
        let generation = state.generation;
        let session = state.sessions.entry(input.session_id.clone()).or_default();
        if session.active.is_some() {
            return Err("当前会话仍在执行任务，请先取消或等待完成".into());
        }
        if !session.used.insert(input.request_id.clone()) {
            return Err("助手请求 ID 已使用，请创建新的请求".into());
        }
        session.active = Some(input.request_id.clone());
        Ok(Ticket {
            started: Instant::now(),
            session: input.session_id.clone(),
            request: input.request_id.clone(),
            generation,
            epoch: session.epoch,
        })
    }
    fn valid(state: &State, ticket: &Ticket) -> bool {
        state.generation == ticket.generation
            && state.sessions.get(&ticket.session).is_some_and(|s| {
                s.epoch == ticket.epoch && s.active.as_ref() == Some(&ticket.request)
            })
    }
    pub(super) fn check(&self, ticket: &Ticket) -> Result<(), String> {
        if Self::valid(&*self.lock()?, ticket) {
            Ok(())
        } else {
            Err("助手任务已取消或因项目库、API 配置变化而失效，请重新发送".into())
        }
    }
    pub(super) fn finish(&self, ticket: &Ticket) {
        if let Ok(mut state) = self.lock() {
            if Self::valid(&state, ticket) {
                state.sessions.get_mut(&ticket.session).unwrap().active = None;
            }
        }
    }
    pub(super) fn propose(
        &self,
        ticket: &Ticket,
        field: &str,
        items: &[Item],
    ) -> Result<String, String> {
        let mut state = self.lock()?;
        if !Self::valid(&state, ticket) {
            return Err("助手任务已取消或失效，未生成可保存建议".into());
        }
        let id = uuid::Uuid::new_v4().to_string();
        state.proposals.insert(
            id.clone(),
            Proposal {
                session: ticket.session.clone(),
                request: ticket.request.clone(),
                generation: ticket.generation,
                epoch: ticket.epoch,
                created: Instant::now(),
                field: field.into(),
                snapshots: items
                    .iter()
                    .filter(|item| {
                        if field == "description" {
                            item.description.is_some()
                        } else {
                            item.category.is_some()
                        }
                    })
                    .map(|i| i.project.clone())
                    .collect(),
                applied: None,
            },
        );
        Ok(id)
    }
    pub fn cancel(&self, session_id: &str, request_id: &str) -> Result<(), String> {
        let mut state = self.lock()?;
        let session = state.sessions.entry(session_id.into()).or_default();
        session.used.insert(request_id.into());
        if session.active.as_deref() == Some(request_id) {
            session.epoch += 1;
            session.active = None;
        }
        state
            .proposals
            .retain(|_, p| p.session != session_id || p.request != request_id);
        Ok(())
    }
    pub fn clear(&self, session_id: &str) -> Result<(), String> {
        let mut state = self.lock()?;
        let session = state.sessions.entry(session_id.into()).or_default();
        session.epoch += 1;
        session.active = None;
        state.proposals.retain(|_, p| p.session != session_id);
        Ok(())
    }
    pub fn mutate<T>(&self, change: impl FnOnce() -> Result<T, String>) -> Result<T, String> {
        let mut state = self.lock()?;
        let result = change()?;
        state.generation += 1;
        state.proposals.clear();
        for session in state.sessions.values_mut() {
            session.epoch += 1;
            session.active = None;
        }
        Ok(result)
    }
    pub fn apply(&self, db: &Database, input: ApplyRequest) -> Result<Vec<Project>, String> {
        let mut state = self.lock()?;
        let generation = state.generation;
        let epoch = state
            .sessions
            .get(&input.session_id)
            .map(|s| s.epoch)
            .ok_or("助手会话已失效")?;
        let proposal = state
            .proposals
            .get_mut(&input.proposal_id)
            .ok_or("助手建议已失效或不存在，请重新生成")?;
        if proposal.session != input.session_id
            || proposal.generation != generation
            || proposal.epoch != epoch
            || proposal.created.elapsed() >= Duration::from_secs(600)
        {
            return Err("助手建议已过期或失效，请重新生成".into());
        }
        if let Some((changes, projects)) = &proposal.applied {
            if *changes != input.changes {
                return Err("该建议已经保存，不能重放不同的修改".into());
            }
            return Ok(projects.clone());
        }
        let result =
            db.apply_assistant_patch(&proposal.field, &proposal.snapshots, &input.changes)?;
        proposal.applied = Some((input.changes, result.clone()));
        Ok(result)
    }
}

#[cfg(test)]
mod lifecycle_tests {
    use super::*;
    fn input() -> Request {
        Request {
            session_id: "expiration".into(),
            request_id: "request".into(),
            prompt: "整理分类".into(),
            scope_ids: vec![],
            reference_time: "2026-10-15T00:00:00Z".into(),
            history: vec![],
            context_ids: vec![],
        }
    }
    #[test]
    fn assistant_expired_proposal_and_cleared_session_are_unusable() {
        let root = tempfile::tempdir().unwrap();
        let db = Database::new(root.path().join("test.sqlite")).unwrap();
        let engine = Engine::default();
        let req = input();
        let ticket = engine.begin(&req).unwrap();
        let proposal = engine.propose(&ticket, "category", &[]).unwrap();
        engine.finish(&ticket);
        engine
            .inner
            .lock()
            .unwrap()
            .proposals
            .get_mut(&proposal)
            .unwrap()
            .created = Instant::now() - Duration::from_secs(601);
        let result = engine.apply(
            &db,
            ApplyRequest {
                session_id: req.session_id.clone(),
                proposal_id: proposal,
                changes: vec![],
            },
        );
        assert!(result.unwrap_err().contains("过期"));
        engine.clear(&req.session_id).unwrap();
        assert!(engine.inner.lock().unwrap().proposals.is_empty());
        assert!(engine.check(&ticket).is_err());
    }
    #[test]
    fn assistant_cancel_before_worker_starts_prevents_reconstruction() {
        let engine = Engine::default();
        let req = input();
        let ticket = engine.begin(&req).unwrap();
        engine.cancel(&req.session_id, &req.request_id).unwrap();
        assert!(engine.check(&ticket).is_err());
        assert!(engine.propose(&ticket, "category", &[]).is_err());
        assert!(engine.begin(&req).is_err());
    }
}
