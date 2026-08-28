//! SHA-256 与 SHA256SUMS（LLVM 发行格式：`<hash>  <rel>`）
use sha2::{Digest, Sha256};
use std::fs;
use std::io::Write;
use std::path::Path;

pub fn sha256_file(path: &Path) -> Result<String, String> {
    let data = fs::read(path).map_err(|e| format!("读取 {}: {e}", path.display()))?;
    let mut h = Sha256::new();
    h.update(&data);
    Ok(hex::encode(h.finalize()))
}

pub fn write_sums(out_dir: &Path, rels: &[&str]) -> Result<bool, String> {
    let mut lines = String::new();
    for rel in rels {
        let full = out_dir.join(rel);
        if !full.exists() { continue; }
        let h = sha256_file(&full)?;
        lines.push_str(&format!("{h}  {rel}\n"));
    }
    if lines.is_empty() { return Ok(false); }
    let mut f = fs::File::create(out_dir.join("SHA256SUMS"))
        .map_err(|e| format!("创建 SHA256SUMS: {e}"))?;
    f.write_all(lines.as_bytes()).map_err(|e| format!("写 SHA256SUMS: {e}"))?;
    Ok(true)
}