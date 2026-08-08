//! 配置管理：序列化本地 AI CLI 列表。
//!
//! 配置文件保存在平台配置目录下的 `ai-cli-hub/config.json`。

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

/// 一个可启动的 AI CLI（或任意终端命令）实例。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CliEntry {
    /// 侧边栏显示的名称，如 "Codex" / "Claude"
    pub name: String,
    /// 启动命令，如 `codex` / `claude` / `powershell.exe`
    pub command: String,
    /// 附加参数
    #[serde(default)]
    pub args: Vec<String>,
    /// 工作目录；None 表示用户主目录
    #[serde(default)]
    pub cwd: Option<PathBuf>,
    /// 附加环境变量
    #[serde(default)]
    pub env: BTreeMap<String, String>,
}

/// 终端主题设置。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ThemeSettings {
    /// 暗色主题预设 (UI 主题)
    #[serde(default)]
    pub dark: bool,
    /// 终端配色方案名称 (例如 "Campbell", "One Half Dark")
    #[serde(default = "default_color_scheme")]
    pub color_scheme: String,
    /// 自定义背景色（覆盖预设）
    #[serde(default)]
    pub background: Option<[u8; 3]>,
    /// 自定义前景色（覆盖预设）
    #[serde(default)]
    pub foreground: Option<[u8; 3]>,
    /// 自定义侧边栏选中卡片颜色
    #[serde(default)]
    pub sidebar_card_color: Option<[u8; 3]>,
    /// 是否开启全局玻璃质感（半透明背景）
    #[serde(default)]
    pub glassmorphism: bool,
}

fn default_color_scheme() -> String {
    "Campbell".to_string()
}

impl Default for ThemeSettings {
    fn default() -> Self {
        Self {
            dark: false,
            color_scheme: default_color_scheme(),
            background: None,
            foreground: None,
            sidebar_card_color: None,
            glassmorphism: false,
        }
    }
}

/// 整个应用的配置。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    #[serde(default = "default_entries")]
    pub clis: Vec<CliEntry>,
    #[serde(default)]
    pub theme: ThemeSettings,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            clis: default_entries(),
            theme: ThemeSettings::default(),
        }
    }
}

/// 默认的 CLI 列表。
pub fn default_entries() -> Vec<CliEntry> {
    vec![
        CliEntry {
            name: "Codex CLI".to_string(),
            command: "codex".to_string(),
            args: Vec::new(),
            cwd: None,
            env: BTreeMap::new(),
        },
        CliEntry {
            name: "Claude CLI".to_string(),
            command: "claude".to_string(),
            args: Vec::new(),
            cwd: None,
            env: BTreeMap::new(),
        },
        CliEntry {
            name: "Terminal".to_string(),
            command: default_shell(),
            args: Vec::new(),
            cwd: None,
            env: BTreeMap::new(),
        },
    ]
}

/// 平台默认 shell。
pub fn default_shell() -> String {
    if cfg!(windows) {
        "powershell.exe".to_string()
    } else {
        std::env::var("SHELL").unwrap_or_else(|_| "/bin/sh".to_string())
    }
}

/// 配置文件路径。
pub fn config_path() -> PathBuf {
    directories::BaseDirs::new()
        .map(|d| d.config_dir().join("ai-cli-hub").join("config.json"))
        .unwrap_or_else(|| PathBuf::from("config.json"))
}

impl AppConfig {
    pub fn load() -> Self {
        let path = config_path();
        match std::fs::read_to_string(&path) {
            Ok(text) => match serde_json::from_str(&text) {
                Ok(cfg) => cfg,
                Err(e) => {
                    eprintln!("[config] 解析失败，使用默认配置: {e}");
                    Self::default()
                }
            },
            Err(_) => Self::default(),
        }
    }

    #[allow(dead_code)] // Phase 2 配置编辑使用
    pub fn save(&self) -> Result<()> {
        let path = config_path();
        let dir = path.parent().context("配置文件无父目录")?;
        std::fs::create_dir_all(dir).with_context(|| format!("创建配置目录 {dir:?}"))?;
        let text = serde_json::to_string_pretty(self)?;
        std::fs::write(&path, text).with_context(|| format!("写入配置 {path:?}"))
    }

    /// 导出到指定路径（测试/调试用）。
    #[allow(dead_code)] // 调试导出
    pub fn save_to(&self, path: &Path) -> Result<()> {
        let text = serde_json::to_string_pretty(self)?;
        std::fs::write(path, text)?;
        Ok(())
    }
}
