//! 领域层（端口）—— 零 UI 依赖，全部可单测。
//!
//! 模块：
//! - `manifest`  : 安装清单模型（serde，additive 演进契约）
//! - `preset`    : 预设/负荷枚举与解析（未知值回退默认）
//! - `builder`   : 构建流水线（收集→payload zip→组装 setup→校验和）
//! - `checksum`  : SHA-256 与 SHA256SUMS（LLVM 发行格式）
//! - `appender`  : 自解压组装/提取（[payload][marker][len] 尾随载荷）

pub mod manifest;
pub mod preset;
pub mod checksum;
pub mod appender;
pub mod builder;