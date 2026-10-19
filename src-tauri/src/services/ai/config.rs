use serde::{Deserialize, Serialize};
use std::{fs, io::Write, path::PathBuf, sync::{Arc, Mutex}};
use super::secrets;

#[derive(Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StoredConfig {
    pub base_url: String,
    pub model: String,
    #[serde(default)]
    protected_key: Vec<u8>,
}
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PublicConfig { pub base_url: String, pub model: String, pub has_api_key: bool }
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConfigInput { pub base_url: String, pub model: String, pub api_key: String, pub clear_api_key: bool }
pub struct ConnectionConfig { pub base_url: String, pub model: String, pub key: String }

#[derive(Clone)]
pub struct Store { path: PathBuf, lock: Arc<Mutex<()>> }
impl Store {
    pub fn new(path: PathBuf) -> Self { Self { path, lock: Arc::new(Mutex::new(())) } }
    pub fn load(&self) -> Result<StoredConfig, String> {
        let _guard = self.lock.lock().map_err(|_| "API 配置锁异常")?;
        self.read()
    }
    fn read(&self) -> Result<StoredConfig, String> {
        match fs::metadata(&self.path) {
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(StoredConfig::default()),
            Ok(meta) if meta.len() <= 64 * 1024 => {},
            _ => return Err("API 配置无法读取或文件过大".into()),
        }
        serde_json::from_slice(&fs::read(&self.path).map_err(|_| "API 配置无法读取")?).map_err(|_| "API 配置损坏，请检查配置文件".into())
    }
    pub fn public(&self) -> Result<PublicConfig, String> { Ok(self.load()?.public()) }
    // 仅解析当前表单和已存密钥，不改动磁盘配置。
    pub fn preview(&self, input: ConfigInput) -> Result<ConnectionConfig, String> {
        let base_url = input.base_url.trim().trim_end_matches('/').to_string();
        endpoint(&base_url)?;
        let model = input.model.trim().to_string();
        if model.chars().count() > 200 || model.chars().any(char::is_control) { return Err("模型名称格式无效".into()); }
        let typed = input.api_key.trim();
        if typed.len() > 4096 || typed.chars().any(char::is_control) { return Err("API Key 格式无效".into()); }
        let key = if !typed.is_empty() { typed.to_string() } else if input.clear_api_key { String::new() } else { self.load()?.key()? };
        Ok(ConnectionConfig { base_url, model, key })
    }
    pub fn save(&self, input: ConfigInput) -> Result<PublicConfig, String> {
        let _guard = self.lock.lock().map_err(|_| "API 配置锁异常")?;
        let mut config = self.read()?;
        let base_url = input.base_url.trim().trim_end_matches('/').to_string();
        let model = input.model.trim().to_string();
        if !base_url.is_empty() || !model.is_empty() {
            endpoint(&base_url)?;
            if model.is_empty() || model.chars().count() > 200 || model.chars().any(char::is_control) { return Err("请填写有效的模型名称（最多 200 个字符）".into()); }
        }
        let key = input.api_key.trim();
        if key.len() > 4096 || key.chars().any(char::is_control) { return Err("API Key 格式无效".into()); }
        if input.clear_api_key { config.protected_key.clear(); }
        if !key.is_empty() { config.protected_key = secrets::transform(key.as_bytes(), true)?; }
        config.base_url = base_url; config.model = model;
        let parent = self.path.parent().ok_or("API 配置目录无效")?;
        fs::create_dir_all(parent).map_err(|_| "无法创建 API 配置目录")?;
        let temporary = parent.join(format!("ai-{}.tmp", uuid::Uuid::new_v4()));
        let result = (|| {
            let mut file = fs::OpenOptions::new().write(true).create_new(true).open(&temporary).map_err(|_| "无法写入 API 配置")?;
            file.write_all(&serde_json::to_vec(&config).map_err(|_| "API 配置序列化失败")?).map_err(|_| "无法写入 API 配置")?;
            file.sync_all().map_err(|_| "API 配置写入失败")?;
            drop(file);
            fs::rename(&temporary, &self.path).map_err(|_| "无法替换 API 配置")?;
            Ok(config.public())
        })();
        if result.is_err() { let _ = fs::remove_file(&temporary); }
        result
    }
}
impl StoredConfig {
    fn public(&self) -> PublicConfig { PublicConfig { base_url: self.base_url.clone(), model: self.model.clone(), has_api_key: !self.protected_key.is_empty() } }
    pub fn key(&self) -> Result<String, String> {
        if self.protected_key.is_empty() { return Ok(String::new()); }
        String::from_utf8(secrets::transform(&self.protected_key, false)?).map_err(|_| "API Key 数据无效，请重新配置".into())
    }
}

pub fn endpoint(base: &str) -> Result<reqwest::Url, String> {
    if base.len() > 2048 { return Err("API 地址过长".into()); }
    let mut url = reqwest::Url::parse(base).map_err(|_| "请输入完整 API 地址")?;
    let host = url.host_str().ok_or("API 地址缺少主机名")?;
    let local = ["localhost", "127.0.0.1", "[::1]", "::1"].contains(&host);
    if url.scheme() != "https" && !(url.scheme() == "http" && local) { return Err("远程 API 必须使用 HTTPS；HTTP 仅支持本机模型服务".into()); }
    if !url.username().is_empty() || url.password().is_some() || url.query().is_some() || url.fragment().is_some() { return Err("API 地址不能包含账户、密钥、查询参数或片段，请单独填写 API Key".into()); }
    let path = url.path().trim_end_matches('/');
    if !path.ends_with("/chat/completions") { url.set_path(&format!("{}/chat/completions", path)); }
    Ok(url)
}
