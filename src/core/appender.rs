//! 自解压尾随载荷：[payload][marker "TIWMARK"][Int64 payload 长度]
//! setup.exe 从文件尾读长度 → 校验 marker → 定位 payload（与 C# 原型同构）
use std::fs;
use std::io::Write;
use std::path::Path;

const MARKER: &[u8] = b"TIWMARK";
const TAIL: usize = MARKER.len() + 8;

pub fn assemble(template: &Path, payload: &Path, out: &Path) -> Result<(), String> {
    let tpl = fs::read(template).map_err(|e| format!("读模板 {}: {e}", template.display()))?;
    let p = fs::read(payload).map_err(|e| format!("读 payload {}: {e}", payload.display()))?;
    let mut f = fs::File::create(out).map_err(|e| format!("创建 {}: {e}", out.display()))?;
    f.write_all(&tpl).map_err(|e| e.to_string())?;
    f.write_all(&p).map_err(|e| e.to_string())?;
    f.write_all(MARKER).map_err(|e| e.to_string())?;
    f.write_all(&(p.len() as u64).to_le_bytes()).map_err(|e| e.to_string())?;
    Ok(())
}

/// 提取自身载荷到 Vec；校验 marker 与长度；失败返回 Err
pub fn extract_self(path: &Path) -> Result<Vec<u8>, String> {
    let all = fs::read(path).map_err(|e| format!("读 {}: {e}", path.display()))?;
    let total = all.len();
    if total < TAIL { return Err("载荷段缺失（文件过短）".into()); }
    let len = u64::from_le_bytes(all[total - 8..].try_into().unwrap()) as usize;
    let mark_off = total - TAIL;
    if len == 0 || len > mark_off { return Err("载荷长度非法".into()); }
    if &all[mark_off..mark_off + MARKER.len()] != MARKER {
        return Err("载荷标志不匹配（非 tiwi 安装包）".into());
    }
    let payload_start = mark_off - len;
    let mut out = Vec::with_capacity(len);
    out.extend_from_slice(&all[payload_start..mark_off]);
    Ok(out)
}

/// 通用读文件为字节（供 builder 校验模板）
pub fn read_all(path: &Path) -> Result<Vec<u8>, String> {
    fs::read(path).map_err(|e| format!("读 {}: {e}", path.display()))
}