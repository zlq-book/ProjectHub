use super::config::{endpoint, ConnectionConfig};
use reqwest::blocking::{Client, RequestBuilder};
use serde::Serialize;
use serde_json::{json, Value};
use std::{io::Read, time::{Duration, Instant}};

fn client() -> Result<Client, String> {
    Client::builder().connect_timeout(Duration::from_secs(10)).timeout(Duration::from_secs(30))
        .redirect(reqwest::redirect::Policy::none()).build().map_err(|_| "无法初始化 API 客户端".into())
}
fn read(request: RequestBuilder, key: &str) -> Result<Value, String> {
    let request = if key.is_empty() { request } else { request.bearer_auth(key) };
    let response = request.send().map_err(|e| if e.is_timeout() { "API 请求超时（30 秒）" } else { "无法连接 API，请检查地址、网络与证书" })?;
    let status = response.status();
    if !status.is_success() { return Err(match status.as_u16() {
        401 | 403 => "AI 服务拒绝授权，请检查 API Key 和权限".into(),
        404 => "接口或模型不存在，请检查 API 地址与模型名称；部分服务不支持模型列表，可手动填写".into(),
        429 => "请求过于频繁或额度不足，请稍后重试".into(),
        code => format!("AI 服务返回 HTTP {}，请检查配置", code),
    }); }
    let mut bytes = Vec::new();
    response.take(1024 * 1024 + 1).read_to_end(&mut bytes).map_err(|_| "API 响应读取失败或超时")?;
    if bytes.len() > 1024 * 1024 { return Err("API 响应过大（超过 1 MiB）".into()); }
    serde_json::from_slice(&bytes).map_err(|_| "API 未返回有效 JSON，请检查接口兼容性".into())
}

pub fn models(config: ConnectionConfig) -> Result<Vec<String>, String> {
    let mut url = endpoint(&config.base_url)?;
    let base = url.path().trim_end_matches('/').strip_suffix("/chat/completions").ok_or("模型接口地址无效")?;
    url.set_path(&format!("{}/models", base));
    let value = read(client()?.get(url), &config.key)?;
    let data = value["data"].as_array().ok_or("模型列表格式无效，需要 data 数组；也可手动填写模型")?;
    let mut models: Vec<String> = data.iter().filter_map(|item| item["id"].as_str())
        .map(str::trim).filter(|id| !id.is_empty() && id.chars().count() <= 200 && !id.chars().any(char::is_control))
        .take(5000).map(str::to_string).collect();
    models.sort(); models.dedup();
    if models.is_empty() { return Err("服务未返回可用模型，请手动填写模型名称".into()); }
    Ok(models)
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TestResult { pub latency_ms: u64, pub model: String }
pub fn test(config: ConnectionConfig) -> Result<TestResult, String> {
    if config.model.is_empty() { return Err("请先选择或填写模型名称".into()); }
    let url = endpoint(&config.base_url)?;
    let start = Instant::now();
    let value = read(client()?.post(url).json(&json!({
        "model": config.model, "messages": [{"role": "user", "content": "请回复 OK。"}], "max_tokens": 16, "stream": false
    })), &config.key)?;
    // 推理模型可能将少量测试 token 用于 reasoning_content，仍说明对话接口已接受该模型。
    let message = &value["choices"][0]["message"];
    if !["content", "reasoning_content"].iter().any(|key| message[key].as_str().is_some_and(|text| !text.trim().is_empty())) { return Err("服务未返回有效对话结果，请检查模型与接口兼容性".into()); }
    Ok(TestResult { latency_ms: start.elapsed().as_millis().min(u64::MAX as u128) as u64, model: config.model })
}
