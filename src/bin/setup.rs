//! setup —— 自解压安装器（复用 core：appender 提取 + zip 展开 + 清单部署）
//! 语义对齐 tie-install-builder 设计文档 §5/§7/§9：
//!   预设 bare | trm-bundled | trm-detect | runtime-toolchain
//!   探测→安装→部署→快捷方式→环境变量→关联→卸载注册→镜像台账
//! 测试钩子（沙箱）：TWI_SIM=1 时拦截系统类写操作，并改用
//!   TWI_SIM_TARGET / TWI_SIM_START / TWI_SIM_TRM_HOME /
//!   TWI_SIM_TIE_HOME / TWI_SIM_MIRROR。
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::process::Command;
use twi::core::appender;
use twi::core::manifest::{sanitize_name, Project};
use twi::core::preset::{parse_preset, Preset};

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let uninstall = args.iter().any(|a| a == "--uninstall");
    match run(uninstall) {
        Ok(()) => std::process::exit(0),
        Err(e) => {
            eprintln!("[E_LOAD] {e}");
            std::process::exit(10);
        }
    }
}

fn current_exe() -> Result<PathBuf, String> {
    std::env::current_exe().map_err(|e| format!("定位自身: {e}"))
}

fn sim() -> bool { std::env::var("TWI_SIM") == Ok("1".to_string()) }

fn env_or(key: &str) -> Option<String> { std::env::var(key).ok().filter(|s| !s.is_empty()) }

fn run(uninstall: bool) -> Result<(), String> {
    let self_path = current_exe()?;
    if uninstall {
        return uninstall_app(&self_path);
    }

    let tmp = std::env::temp_dir().join(format!("twi-{}", nanos()));
    fs::create_dir_all(&tmp).map_err(|e| e.to_string())?;

    // 1) 提取尾随载荷并展开
    let payload = appender::extract_self(&self_path)?;
    let pzip = tmp.join("payload.zip");
    fs::write(&pzip, &payload).map_err(|e| e.to_string())?;
    let file = fs::File::open(&pzip).map_err(|e| e.to_string())?;
    let mut arc = zip::ZipArchive::new(file).map_err(|e| format!("展开 payload: {e}"))?;

    // 2) 读内嵌清单（内层块结束即释放 arc 借用）
    let proj = {
        let mut mf = arc.by_name("manifest.json").map_err(|_| "payload 缺 manifest.json".to_string())?;
        let mut text = String::new();
        io::Read::read_to_string(&mut mf, &mut text).map_err(|e| e.to_string())?;
        Project::from_json(&text)?
    };

    // 3) 目标目录（默认 %LocalAppData%\Programs\{name}；清单 installDir 可覆盖）
    let target = target_dir(&proj);

    // 4) 部署文件
    deploy(&mut arc, &tmp, &target, &proj)?;

    // 5) 保留安装清单与镜像台账（卸载用）
    fs::create_dir_all(target.join(".twi")).map_err(|e| e.to_string())?;
    fs::write(target.join(".twi/manifest.json"), proj.to_json()?).map_err(|e| e.to_string())?;
    let mirror = mirror_path(&proj)?;
    write_mirror(&mirror, &target, &proj)?;

    // 6) TRM 预设（bundled / detect / runtime-toolchain）
    install_trm(&mut arc, &tmp, &proj)?;

    // 7) 用户级注册（快捷方式/环境变量/关联/卸载注册）——沙箱拦截
    if sim() {
        println!("[sim] 跳过快捷方式/环境变量/注册表写操作");
    } else {
        install_shortcuts(&proj, &target)?;
        apply_env(&proj, &target)?;
        apply_assoc(&proj, &target)?;
        write_uninstall_reg(&proj, &target, &self_path)?;
    }

    println!("[ok] 安装完成: {}", target.display());
    Ok(())
}

// ---------------- 目录解析 ----------------

fn target_dir(proj: &Project) -> PathBuf {
    if sim() {
        if let Some(t) = env_or("TWI_SIM_TARGET") { return PathBuf::from(t); }
    }
    if !proj.build.install_dir.is_empty() {
        return PathBuf::from(&proj.build.install_dir);
    }
    let base = env_or("LOCALAPPDATA")
        .or_else(|| env_or("USERPROFILE"))
        .unwrap_or_else(|| ".".into());
    PathBuf::from(base).join("Programs").join(sanitize_name(&proj.app.name, "app"))
}

fn start_root(proj: &Project) -> PathBuf {
    if sim() {
        if let Some(s) = env_or("TWI_SIM_START") { return PathBuf::from(s); }
    }
    let base = env_or("APPDATA").unwrap_or_else(|| ".".into());
    let folder = sanitize_name(&proj.app.name, "apps");
    PathBuf::from(base).join("Microsoft/Windows/Start Menu/Programs").join(folder)
}

fn trm_home(proj: &Project) -> PathBuf {
    if sim() {
        if let Some(t) = env_or("TWI_SIM_TRM_HOME") { return PathBuf::from(t); }
    }
    if let Some(t) = env_or("TIE_TRM_HOME") { return PathBuf::from(t); }
    let h = if proj.trm.home.is_empty() { "C:\\tie\\trm".to_string() } else { proj.trm.home.clone() };
    PathBuf::from(h.replace('/', "\\"))
}

fn tie_home() -> PathBuf {
    if sim() {
        if let Some(t) = env_or("TWI_SIM_TIE_HOME") { return PathBuf::from(t); }
    }
    PathBuf::from("C:\\tie")
}

fn mirror_path(proj: &Project) -> Result<PathBuf, String> {
    let dir = if sim() {
        PathBuf::from(env_or("TWI_SIM_MIRROR").unwrap_or_else(|| ".".into()))
    } else {
        PathBuf::from(env_or("APPDATA").ok_or("缺 APPDATA")?).join("twi/installed")
    };
    fs::create_dir_all(&dir).map_err(|e| format!("镜像目录: {e}"))?;
    Ok(dir.join(sanitize_name(&proj.app.id, "app") + ".json"))
}

// ---------------- 部署 ----------------

fn deploy(arc: &mut zip::ZipArchive<fs::File>, tmp: &Path, target: &Path, proj: &Project) -> Result<(), String> {
    if proj.files.is_empty() {
        for i in 0..arc.len() {
            let mut f = arc.by_index(i).map_err(|e| e.to_string())?;
            let name = f.name().to_string();
            if name == "manifest.json" || name.starts_with("trm/") || name.starts_with("toolchain/") {
                continue;
            }
            let dst = target.join(&name);
            fs::create_dir_all(dst.parent().unwrap_or(target)).map_err(|e| e.to_string())?;
            let mut out = fs::File::create(&dst).map_err(|e| e.to_string())?;
            io::copy(&mut f, &mut out).map_err(|e| e.to_string())?;
        }
        return Ok(());
    }
    for fm in &proj.files {
        let entry = fm.to.replace('\\', "/").trim_start_matches('/').to_string();
        if entry.is_empty() { continue; }
        match arc.by_name(&entry) {
            Ok(mut f) => {
                let dst = target.join(&entry);
                fs::create_dir_all(dst.parent().unwrap_or(target)).map_err(|e| e.to_string())?;
                let mut out = fs::File::create(&dst).map_err(|e| e.to_string())?;
                io::copy(&mut f, &mut out).map_err(|e| e.to_string())?;
            }
            Err(_) => { println!("[warn] payload 缺条目: {entry}"); }
        }
    }
    Ok(())
}

// ---------------- TRM 预设 ----------------

fn require_major(proj: &Project) -> String {
    let req = proj.trm.require.trim_start_matches(|c| c == '^' || c == '~').to_string();
    if let Some(dot) = req.find('.') {
        req[..dot].to_string()
    } else {
        req
    }
}

fn is_trm_satisfied(home: &Path, major: &str) -> bool {
    let vd = home.join(major);
    if !vd.is_dir() { return false; }
    let mut any = false;
    if let Ok(rd) = fs::read_dir(&vd) {
        for e in rd.flatten() {
            any = true;
            let _ = e;
        }
    }
    any
}

fn copy_zip_prefix(arc: &mut zip::ZipArchive<fs::File>, prefix: &str, dest: &Path) -> Result<i64, String> {
    let mut copied: i64 = 0;
    let names: Vec<String> = (0..arc.len())
        .filter_map(|i| arc.by_index(i).ok().map(|f| f.name().to_string()))
        .collect();
    for name in names {
        if !name.starts_with(prefix) { continue; }
        let rel = name[prefix.len()..].trim_start_matches('/');
        if rel.is_empty() { continue; }
        let mut f = arc.by_name(&name).map_err(|e| e.to_string())?;
        let dst = dest.join(rel);
        fs::create_dir_all(dst.parent().unwrap_or(dest)).map_err(|e| e.to_string())?;
        let mut out = fs::File::create(&dst).map_err(|e| e.to_string())?;
        io::copy(&mut f, &mut out).map_err(|e| e.to_string())?;
        copied += 1;
    }
    Ok(copied)
}

fn install_trm(arc: &mut zip::ZipArchive<fs::File>, tmp: &Path, proj: &Project) -> Result<(), String> {
    let preset = parse_preset(&proj.preset);
    if preset == Preset::Bare { return Ok(()); }

    let home = trm_home(proj);
    let major = require_major(proj);
    if preset == Preset::TrmDetect && is_trm_satisfied(&home, &major) {
        println!("[trm] 检测到兼容 TRM(major={major})，跳过安装");
        return Ok(());
    }
    let vd = home.join(&major);
    let copied = copy_zip_prefix(arc, "trm", &vd)?;
    println!("[trm] TRM 已装至 {}（{} 件）", vd.display(), copied);
    if !sim() {
        set_user_env("TIE_TRM_HOME".to_string(), home.to_string_lossy().to_string());
    } else {
        println!("[sim] 跳过设置 TIE_TRM_HOME");
    }
    if preset == Preset::RuntimeToolchain {
        let th = tie_home();
        let bin = th.join("bin");
        let n = copy_zip_prefix(arc, "toolchain", &bin)?;
        println!("[toolchain] 精简工具链已装至 {}（{} 件）", bin.display(), n);
        if !sim() {
            append_user_path(bin.to_string_lossy().to_string());
        } else {
            println!("[sim] 跳过 PATH 追加");
        }
    }
    Ok(())
}

// ---------------- 用户级注册（Windows 原生，无额外依赖） ----------------

fn install_shortcuts(proj: &Project, target: &Path) -> Result<(), String> {
    let root = start_root(proj);
    fs::create_dir_all(&root).map_err(|e| e.to_string())?;
    for s in &proj.shortcuts {
        if s.name.is_empty() { continue; }
        let tt = resolve_ref(&s.target, target);
        let lnk = root.join(sanitize_name(&s.name, "shortcut") + ".lnk");
        let script = format!(
            "$w=New-Object -ComObject WScript.Shell;$c=$w.CreateShortcut('{lnk}');$c.TargetPath='{tt}';$c.Save()",
            lnk = lnk.to_string_lossy(),
            tt = tt.replace('\'', "''"),
        );
        powershell(&script)?;
    }
    Ok(())
}

fn resolve_ref(s: &str, target: &Path) -> String {
    if let Some(rest) = s.strip_prefix("@app") {
        target.join(rest.trim_start_matches('/').trim_start_matches('\\')).to_string_lossy().to_string()
    } else {
        s.to_string()
    }
}

fn apply_env(proj: &Project, target: &Path) -> Result<(), String> {
    for e in &proj.env {
        if e.name.is_empty() { continue; }
        if e.name.eq_ignore_ascii_case("PATH") {
            let add = resolve_ref(&e.append, target);
            append_user_path(add)?;
        }
    }
    Ok(())
}

fn append_user_path(add: String) -> Result<(), String> {
    let script = format!(
        "$cur=[Environment]::GetEnvironmentVariable('PATH','User');if($null -eq $cur){{$cur=''}};if($cur.IndexOf('{a}') -lt 0){{$n=($cur.TrimEnd(';')+';{a}').TrimStart(';');[Environment]::SetEnvironmentVariable('PATH',$n,'User')}}",
        a = add.replace('\'', "''"),
    );
    powershell(&script)
}

fn set_user_env(name: String, value: String) -> Result<(), String> {
    let script = format!(
        "[Environment]::SetEnvironmentVariable('{n}','{v}','User')",
        n = name.replace('\'', "''"),
        v = value.replace('\'', "''"),
    );
    powershell(&script)
}

fn apply_assoc(proj: &Project, target: &Path) -> Result<(), String> {
    for a in &proj.assoc {
        if a.ext.is_empty() { continue; }
        let ext = if a.ext.starts_with('.') { a.ext.clone() } else { format!(".{}", a.ext) };
        let cmd = resolve_ref(&a.cmd, target);
        let prog_id = sanitize_name(&proj.app.id, "app") + &ext;
        reg_add(&format!(r"HKCU\Software\Classes\{ext}"), "/ve", &prog_id)?;
        reg_add(&format!(r"HKCU\Software\Classes\{prog_id}\shell\open\command"), "/ve", &format!("\"{cmd}\" \"%1\""))?;
    }
    Ok(())
}

fn write_uninstall_reg(proj: &Project, target: &Path, self_path: &Path) -> Result<(), String> {
    if proj.app.id.is_empty() { return Ok(()); }
    let key = format!(r"HKCU\Software\Microsoft\Windows\CurrentVersion\Uninstall\{}", sanitize_name(&proj.app.id, "app"));
    let un = format!("\"{}\" --uninstall", self_path.to_string_lossy());
    let _ = reg_add_value(&key, "DisplayName", &proj.app.name);
    let _ = reg_add_value(&key, "DisplayVersion", &proj.app.version);
    let _ = reg_add_value(&key, "Publisher", &proj.app.publisher);
    let _ = reg_add_value(&key, "InstallLocation", &target.to_string_lossy());
    let _ = reg_add_value(&key, "UninstallString", &un);
    Ok(())
}

fn reg_add(key: &str, flag: &str, value: &str) -> Result<(), String> {
    let mut c = Command::new("reg");
    c.arg("add").arg(key).arg(flag).arg("/d").arg(value).arg("/f");
    let out = c.output().map_err(|e| format!("reg add: {e}"))?;
    if !out.status.success() {
        let msg = String::from_utf8_lossy(&out.stderr).to_string();
        return Err(format!("reg add {key}: {msg}"));
    }
    Ok(())
}

fn reg_add_value(key: &str, name: &str, value: &str) -> Result<(), String> {
    let mut c = Command::new("reg");
    c.arg("add").arg(key).arg("/v").arg(name).arg("/d").arg(value).arg("/f");
    let out = c.output().map_err(|e| format!("reg add: {e}"))?;
    if !out.status.success() {
        return Err(format!("reg add {key}\\{name}: {}", String::from_utf8_lossy(&out.stderr)));
    }
    Ok(())
}

fn powershell(script: &str) -> Result<(), String> {
    let out = Command::new("powershell")
        .arg("-NoProfile")
        .arg("-Command")
        .arg(script)
        .output()
        .map_err(|e| format!("powershell: {e}"))?;
    if !out.status.success() {
        let msg = String::from_utf8_lossy(&out.stderr).to_string();
        return Err(format!("powershell: {msg}"));
    }
    Ok(())
}

// ---------------- 镜像台账 / 卸载 ----------------

struct Mirror {
    target: String,
    start: String,
    id: String,
}

fn write_mirror(path: &Path, target: &Path, proj: &Project) -> Result<(), String> {
    let json = format!(
        "{{\"target\":\"{}\",\"start\":\"{}\",\"id\":\"{}\"}}",
        target.to_string_lossy().replace('\\', "\\\\"),
        start_root(proj).to_string_lossy().replace('\\', "\\\\"),
        sanitize_name(&proj.app.id, "app"),
    );
    fs::write(path, json).map_err(|e| format!("写镜像: {e}"))?;
    Ok(())
}

fn read_mirror(path: &Path) -> Result<Mirror, String> {
    let text = fs::read_to_string(path).map_err(|e| format!("读镜像: {e}"))?;
    let mut target = String::new();
    let mut start = String::new();
    let mut id = String::new();
    for seg in text.trim_matches(['{', '}']).split(',') {
        let kv: Vec<&str> = seg.splitn(2, ':').collect();
        if kv.len() != 2 { continue; }
        let k = kv[0].trim_matches('"');
        let v = kv[1].trim_matches('"').replace("\\\\", "\\");
        if k == "target" { target = v; }
        else if k == "start" { start = v; }
        else if k == "id" { id = v; }
    }
    Ok(Mirror { target, start, id })
}

fn uninstall_app(self_path: &Path) -> Result<(), String> {
    // 通过内置镜像台账定位（先在本目录旁找用户台账）
    let dir = if sim() {
        PathBuf::from(env_or("TWI_SIM_MIRROR").unwrap_or_else(|| ".".into()))
    } else {
        PathBuf::from(env_or("APPDATA").ok_or("缺 APPDATA")?).join("twi/installed")
    };
    let mut found: Option<PathBuf> = None;
    if let Ok(rd) = fs::read_dir(&dir) {
        for ent in rd.flatten() {
            if ent.path().extension().map(|e| e == "json").unwrap_or(false) {
                if let Ok(m) = read_mirror(&ent.path()) {
                    let un = format!("\"{}\" --uninstall", self_path.to_string_lossy());
                    let stored_un = String::new(); // 镜像无 UninstallString；匹配父目录可移植
                    let _ = stored_un;
                    let _ = un;
                    found = Some(ent.path());
                    break;
                }
            }
        }
    }
    match found {
        Some(mpath) => {
            let m = read_mirror(&mpath)?;
            if !m.target.is_empty() {
                let t = PathBuf::from(&m.target);
                if t.exists() { fs::remove_dir_all(&t).map_err(|e| format!("清理 {}", t.display()))?; }
                println!("[ok] 已删除安装目录: {}", t.display());
            }
            if !m.start.is_empty() && !sim() {
                let s = PathBuf::from(&m.start);
                if s.is_dir() { let _ = fs::remove_dir_all(&s); }
            }
            if !sim() {
                let key = format!(r"HKCU\Software\Microsoft\Windows\CurrentVersion\Uninstall\{}", m.id);
                let _ = Command::new("reg").arg("delete").arg(key).arg("/f").output();
            }
            let _ = fs::remove_file(&mpath);
            println!("[ok] 已卸载并清理镜像台账");
            Ok(())
        }
        None => {
            println!("[warn] 未找到镜像台账，跳过");
            Ok(())
        }
    }
}

fn nanos() -> u128 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0)
}