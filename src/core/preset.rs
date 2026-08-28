//! 预设/负荷语义 —— 未知枚举值回退默认（additive：新增预设=新字符串，旧值语义不变）
use crate::core::manifest::{Payload, Project};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Preset { Bare, TrmBundled, TrmDetect, RuntimeToolchain }

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PayloadFormat { Binary, TieIr, LlvmIr }

pub fn parse_preset(s: &str) -> Preset {
    match s {
        "trm-bundled" => Preset::TrmBundled,
        "trm-detect" => Preset::TrmDetect,
        "runtime-toolchain" => Preset::RuntimeToolchain,
        _ => Preset::Bare,
    }
}

pub fn parse_payload_format(p: &Payload) -> PayloadFormat {
    match p.format.as_str() {
        "tieir" => PayloadFormat::TieIr,
        "llvmir" => PayloadFormat::LlvmIr,
        _ => PayloadFormat::Binary,
    }
}

/// 预设是否需要 TRM 进场（非 bare）
pub fn needs_trm(p: &Project) -> bool {
    parse_preset(&p.preset) != Preset::Bare
}