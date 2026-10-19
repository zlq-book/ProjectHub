use crate::services::ai::config::{self, StoredConfig};
use serde_json::{json, Value};
use std::{
    io::Read,
    time::{Duration, Instant},
};
pub struct Budget {
    started: Instant,
    requests: usize,
    tools: usize,
    truncated: bool,
}
fn wire(model: &str, system: &str, payload: &Value) -> Value {
    json!({"model":model,"messages":[{"role":"system","content":system},{"role":"user","content":payload.to_string()}],"temperature":0.1,"max_tokens":8192,"stream":false})
}
fn shorten(value: &mut Value, minimum: usize) -> bool {
    let Some(text) = value.as_str() else {
        return false;
    };
    let length = text.chars().count();
    if length <= minimum {
        return false;
    }
    *value = json!(text
        .chars()
        .take((length / 2).max(minimum))
        .collect::<String>());
    true
}
fn shrink(payload: &mut Value) -> bool {
    if let Some(contexts) = payload["contexts"].as_array_mut() {
        for context in contexts {
            if let Some(files) = context["context"]["files"].as_array_mut() {
                for file in files {
                    if shorten(&mut file["content"], 128) {
                        file["truncated"] = json!(true);
                        return true;
                    }
                }
            }
        }
    }
    if let Some(projects) = payload["projects"].as_array_mut() {
        for project in projects {
            for name in ["description", "name"] {
                if shorten(&mut project[name], 16) {
                    project["truncated"] = json!(true);
                    return true;
                }
            }
            for name in ["techStack", "tags"] {
                if let Some(items) = project[name].as_array_mut() {
                    if items.len() > 1 {
                        items.pop();
                        project["truncated"] = json!(true);
                        return true;
                    }
                    if let Some(item) = items.first_mut() {
                        if shorten(item, 16) {
                            project["truncated"] = json!(true);
                            return true;
                        }
                    }
                }
            }
        }
    }
    if let Some(history) = payload["history"].as_array_mut() {
        for message in history {
            if shorten(&mut message["content"], 128) {
                return true;
            }
        }
    }
    false
}
impl Budget {
    pub fn truncated(&self) -> bool {
        self.truncated
    }
    pub fn new(started: Instant) -> Self {
        Self {
            started,
            requests: 0,
            tools: 0,
            truncated: false,
        }
    }
    pub fn remaining(&self) -> Result<Duration, String> {
        Duration::from_secs(90)
            .checked_sub(self.started.elapsed())
            .filter(|d| !d.is_zero())
            .ok_or_else(|| "助手任务已超过 90 秒，请缩小范围后重试".into())
    }
    pub fn tool(&mut self) -> Result<(), String> {
        self.remaining()?;
        self.tools += 1;
        if self.tools > 8 {
            Err("助手工具调用超过每轮 8 次上限".into())
        } else {
            Ok(())
        }
    }
    pub fn chat(
        &mut self,
        config: &StoredConfig,
        system: &str,
        payload: &Value,
    ) -> Result<String, String> {
        let timeout = self.remaining()?.min(Duration::from_secs(60));
        self.requests += 1;
        if self.requests > 4 {
            return Err("助手模型请求超过每轮 4 次上限".into());
        }
        if config.base_url.is_empty() || config.model.is_empty() {
            return Err("请先配置 API 地址与模型后再使用 AI 助手".into());
        }
        let client = reqwest::blocking::Client::builder()
            .connect_timeout(timeout.min(Duration::from_secs(10)))
            .timeout(timeout)
            .redirect(reqwest::redirect::Policy::none())
            .build()
            .map_err(|_| "无法初始化 AI 请求客户端")?;
        let mut bounded = payload.clone();
        let body = loop {
            let body = wire(&config.model, system, &bounded);
            if serde_json::to_vec(&body)
                .map_err(|_| "助手请求序列化失败")?
                .len()
                <= 64 * 1024
            {
                break body;
            }
            if !shrink(&mut bounded) {
                return Err("助手请求无法压缩到 64 KiB，请缩短输入或缩小项目范围".into());
            }
            bounded["payloadTruncated"] = json!(true);
            self.truncated = true;
            self.remaining()?;
        };
        let mut request = client
            .post(config::endpoint(&config.base_url)?)
            .timeout(self.remaining()?.min(Duration::from_secs(60)))
            .json(&body);
        let key = config.key()?;
        if !key.is_empty() {
            request = request.bearer_auth(key);
        }
        let response = request.send().map_err(|error| {
            if error.is_timeout() {
                "AI 助手请求超时，请重试"
            } else {
                "模型服务连接失败，请检查网络与 API 配置"
            }
        })?;
        if !response.status().is_success() {
            return Err(match response.status().as_u16() {
                401 | 403 => "AI 服务拒绝授权，请检查 API Key".into(),
                404 => "AI 接口或模型不存在".into(),
                429 => "AI 服务限流或额度不足，请稍后重试".into(),
                code => format!("AI 服务返回 HTTP {code}"),
            });
        }
        let mut bytes = vec![];
        response
            .take(256 * 1024 + 1)
            .read_to_end(&mut bytes)
            .map_err(|_| "AI 响应读取失败或超时")?;
        if bytes.len() > 256 * 1024 {
            return Err("AI 助手响应超过 256 KiB".into());
        }
        let value: Value = serde_json::from_slice(&bytes).map_err(|_| "AI 接口未返回有效 JSON")?;
        if value["choices"][0]["finish_reason"] == "length" {
            return Err("AI 助手输出被截断，请缩小范围".into());
        }
        value["choices"][0]["message"]["content"]
            .as_str()
            .map(str::to_owned)
            .ok_or_else(|| "AI 响应缺少文本结果".into())
    }
}
