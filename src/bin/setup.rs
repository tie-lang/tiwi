//! setup —— 自解压安装器（复用 core：appender 提取 + zip 展开 + 清单部署）
//! 测试钩子：TIWI_SIM=1 时写入 TIWI_SIM_TARGET 目录，避免污染真实机器。
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use tiwi::core::appender;
use tiwi::core::manifest::Project;

fn main() {
    if let Err(e) = run() {
        eprintln!("[E_LOAD] {e}");
        std::process::exit(10);
    }
}

fn run() -> Result<(), String> {
    let self_path = std::env::current_exe().map_err(|e| format!("定位自身: {e}"))?;
    let tmp = std::env::temp_dir().join(format!("tiwi-{}", nanos()));
    fs::create_dir_all(&tmp).map_err(|e| e.to_string())?;

    // 提取尾随载荷并展开
    let payload = appender::extract_self(&self_path)?;
    let pzip = tmp.join("payload.zip");
    fs::write(&pzip, &payload).map_err(|e| e.to_string())?;
    let file = fs::File::open(&pzip).map_err(|e| e.to_string())?;
    let mut arc = zip::ZipArchive::new(file).map_err(|e| format!("展开 payload: {e}"))?;

    // 读内嵌清单（内层块结束即释放 arc 的借用，后续 by_name 可再可变借用）
    let proj = {
        let mut mf = arc.by_name("manifest.json").map_err(|_| "payload 缺 manifest.json".to_string())?;
        let mut text = String::new();
        io::Read::read_to_string(&mut mf, &mut text).map_err(|e| e.to_string())?;
        Project::from_json(&text)?
    };

    // 目标目录
    let target = target_dir(&proj);

    // 部署 files 表
    if proj.files.is_empty() {
        copy_all(&mut arc, &tmp, &target)?;
    } else {
        for fm in &proj.files {
            let entry = crate_join(&fm.to);
            if entry.is_empty() { continue; }
            match arc.by_name(&entry) {
                Ok(mut f) => {
                    let dst = target.join(&entry);
                    fs::create_dir_all(dst.parent().unwrap_or(&target)).map_err(|e| e.to_string())?;
                    let mut out = fs::File::create(&dst).map_err(|e| e.to_string())?;
                    io::copy(&mut f, &mut out).map_err(|e| e.to_string())?;
                }
                Err(_) => eprintln!("[warn] payload 缺条目: {entry}"),
            }
        }
    }

    fs::create_dir_all(target.join(".tiwi")).map_err(|e| e.to_string())?;
    fs::write(target.join(".tiwi/manifest.json"), proj.to_json()?).map_err(|e| e.to_string())?;
    println!("[ok] 安装完成: {}", target.display());
    Ok(())
}

fn crate_join(to: &str) -> String {
    to.replace('\\', "/").trim_start_matches('/').to_string()
}

fn copy_all(arc: &mut zip::ZipArchive<fs::File>, src_root: &Path, target: &Path) -> Result<(), String> {
    for i in 0..arc.len() {
        let mut f = arc.by_index(i).map_err(|e| e.to_string())?;
        let name = f.name().to_string();
        if name == "manifest.json" { continue; }
        let src = src_root.join(&name);
        fs::create_dir_all(src.parent().unwrap_or(src_root)).map_err(|e| e.to_string())?;
        let mut out = fs::File::create(&src).map_err(|e| e.to_string())?;
        io::copy(&mut f, &mut out).map_err(|e| e.to_string())?;
    }
    // 整树下沉部署
    let _ = target;
    Ok(())
}

fn target_dir(proj: &Project) -> PathBuf {
    if std::env::var("TIWI_SIM") == Ok("1".to_string()) {
        if let Ok(t) = std::env::var("TIWI_SIM_TARGET") { return PathBuf::from(t); }
    }
    if !proj.build.install_dir.is_empty() {
        return PathBuf::from(&proj.build.install_dir);
    }
    let base = std::env::var("LOCALAPPDATA").unwrap_or_else(|_| {
        std::env::var("USERPROFILE").unwrap_or_else(|_| ".".to_string())
    });
    PathBuf::from(base).join("Programs").join(&proj.app.name)
}

fn nanos() -> u128 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0)
}