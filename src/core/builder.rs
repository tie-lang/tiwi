//! 构建流水线：收集 → payload zip → 组装 setup.exe → 清单产物 + SHA256SUMS
use crate::core::appender;
use crate::core::checksum;
use crate::core::manifest::{sanitize_name, Project};
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use zip::write::SimpleFileOptions;

/// 归一化安装树内相对路径（防 ".." 逃逸与盘符）
pub fn normalize_to(to: &str) -> String {
    let t = to.replace('\\', "/");
    let t = t.trim_start_matches('/');
    let mut parts = Vec::new();
    for seg in t.split('/') {
        if seg.is_empty() || seg == "." { continue; }
        if seg == ".." { continue; }
        parts.push(seg);
    }
    parts.join("/")
}

#[derive(Debug)]
pub struct BuildReport {
    pub root: String,
    pub payload_zip: PathBuf,
    pub setup_exe: PathBuf,
    pub manifest_out: PathBuf,
}

/// 返回模板路径：project.build.setup_template → 环境变量 → res/setup-template.exe
fn resolve_template(proj: &Project, proj_dir: &Path) -> Option<PathBuf> {
    if !proj.build.setup_template.is_empty() {
        let p = proj_dir.join(&proj.build.setup_template);
        if p.exists() { return Some(p); }
    }
    if let Ok(e) = std::env::var("TIWI_SETUP_TEMPLATE") {
        let p = PathBuf::from(e);
        if p.exists() { return Some(p); }
    }
    if let Ok(exe) = std::env::current_exe() {
        let p = exe.parent().unwrap_or(Path::new(".")).join("res").join("setup-template.exe");
        if p.exists() { return Some(p); }
    }
    for cand in [PathBuf::from("res/setup-template.exe")] {
        if cand.exists() { return Some(cand); }
    }
    None
}

pub fn build(proj: &Project, proj_dir: &Path, out_override: Option<&Path>) -> Result<BuildReport, String> {
    let out_dir: PathBuf = match out_override {
        Some(o) => o.to_path_buf(),
        None => proj_dir.join(&proj.build.out_dir),
    };
    fs::create_dir_all(&out_dir).map_err(|e| format!("创建输出目录: {e}"))?;

    let root = format!("{}-{}", sanitize_name(&proj.app.name, "app"), proj.app.version);
    let payload_zip = out_dir.join(format!("{root}.payload.zip"));
    let setup_exe = out_dir.join(format!("{root}.setup.exe"));
    let manifest_out = out_dir.join(format!("{root}.manifest.json"));

    // 1) payload zip：files 表 from → 条目 to；并内嵌 manifest.json
    {
        let f = fs::File::create(&payload_zip).map_err(|e| format!("创建 payload: {e}"))?;
        let mut zw = zip::ZipWriter::new(f);
        let opts = SimpleFileOptions::default();
        for fm in &proj.files {
            if fm.from.is_empty() { continue; }
            let entry = normalize_to(&fm.to);
            if entry.is_empty() { continue; }
            let src = proj_dir.join(&fm.from);
            let data = fs::read(&src)
                .map_err(|e| format!("读取源 {}: {e}", src.display()))?;
            zw.start_file(&entry, opts)
                .map_err(|e| format!("zip 条目 {entry}: {e}"))?;
            zw.write_all(&data).map_err(|e| e.to_string())?;
        }
        let mjson = proj.to_json()?;
        zw.start_file("manifest.json", opts).map_err(|e| e.to_string())?;
        zw.write_all(mjson.as_bytes()).map_err(|e| e.to_string())?;
        zw.finish().map_err(|e| format!("payload 收尾: {e}"))?;
    }

    // 2) 组装自解压 setup.exe
    let template = resolve_template(proj, proj_dir)
        .ok_or_else(|| "未找到 setup 模板（设 TIWI_SETUP_TEMPLATE 或放 res/setup-template.exe）".to_string())?;
    appender::assemble(&template, &payload_zip, &setup_exe)?;

    // 3) 清单产物
    fs::write(&manifest_out, proj.to_json()?).map_err(|e| format!("写清单产物: {e}"))?;

    // 4) 校验和
    let rels = [root_join(&root, ".payload.zip"), root_join(&root, ".setup.exe"), root_join(&root, ".manifest.json")];
    let sums: Vec<&str> = rels.iter().map(|s| s.as_str()).collect();
    checksum::write_sums(&out_dir, &sums)?;

    Ok(BuildReport { root, payload_zip, setup_exe, manifest_out })
}

fn root_join(root: &str, ext: &str) -> String { format!("{root}{ext}") }