//! 安装清单模型 —— 契约面 §4（format 只增不改；未知字段容忍读取）
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct App {
    pub id: String,
    pub name: String,
    pub version: String,
    pub publisher: String,
}

impl Default for App {
    fn default() -> Self {
        App { id: String::new(), name: "app".into(), version: "1.0.0".into(), publisher: String::new() }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Payload {
    pub format: String,   // binary | tieir | llvmir
    pub compile: String,  // llvmir: target | build
    pub entry: String,
    pub optimize: String,
}

impl Default for Payload {
    fn default() -> Self {
        Payload { format: "binary".into(), compile: "target".into(), entry: String::new(), optimize: "-O2".into() }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Trm {
    pub require: String,
    pub bundle: bool,
    pub home: String,
}

impl Default for Trm {
    fn default() -> Self {
        Trm { require: "^0".into(), bundle: false, home: "C:\\tie\\trm".into() }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Toolchain {
    pub include: Vec<String>,
    pub exclude: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct FileMap {
    pub from: String,
    pub to: String,
    pub exec: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Shortcut {
    pub name: String,
    pub target: String,
    pub args: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct EnvItem {
    pub name: String,
    pub append: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Assoc {
    pub ext: String,
    pub cmd: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Uninstall {
    pub cmd: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct BuildOpt {
    pub out_dir: String,
    pub install_dir: String,
    pub trm_source: String,
    // bundle 真实文件时的本地模板安装器（缺省找 res/setup-template.exe 与 env TIWI_SETUP_TEMPLATE）
    pub setup_template: String,
}

impl Default for BuildOpt {
    fn default() -> Self {
        BuildOpt {
            out_dir: "out".into(),
            install_dir: String::new(),
            trm_source: String::new(),
            setup_template: String::new(),
        }
    }
}

/// 安装清单根（format 首字段，additive 演进起点）
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Project {
    pub format: i64,
    pub app: App,
    pub preset: String,       // bare | trm-bundled | trm-detect | runtime-toolchain
    pub payload: Payload,
    pub trm: Trm,
    pub toolchain: Toolchain,
    pub files: Vec<FileMap>,
    pub shortcuts: Vec<Shortcut>,
    pub env: Vec<EnvItem>,
    pub assoc: Vec<Assoc>,
    pub uninstall: Uninstall,
    pub build: BuildOpt,
}

impl Default for Project {
    fn default() -> Self {
        Project {
            format: 1,
            app: App::default(),
            preset: "bare".into(),
            payload: Payload::default(),
            trm: Trm::default(),
            toolchain: Toolchain::default(),
            files: Vec::new(),
            shortcuts: Vec::new(),
            env: Vec::new(),
            assoc: Vec::new(),
            uninstall: Uninstall::default(),
            build: BuildOpt::default(),
        }
    }
}

impl Project {
    pub fn from_json(text: &str) -> Result<Project, String> {
        serde_json::from_str(text).map_err(|e| format!("清单解析失败: {e}"))
    }
    pub fn to_json(&self) -> Result<String, String> {
        serde_json::to_string_pretty(self).map_err(|e| format!("清单序列化失败: {e}"))
    }
}

/// 幂等文件名（防路径逃逸/非法字符）
pub fn sanitize_name(s: &str, fallback: &str) -> String {
    let t: String = s.chars()
        .map(|c| if c.is_ascii_alphanumeric() || c == '-' || c == '_' || c == '.' { c } else { '_' })
        .collect();
    let t = t.trim().to_string();
    if t.is_empty() { fallback.to_string() } else { t }
}