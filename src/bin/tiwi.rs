//! tiwi 入口：`--build <proj> [--out <dir>]` 走无头 CLI；无参则启动 FLTK GUI。
//! GUI 是 core 的薄适配层（六边形架构的"适配器"）：UI 不持有任何构建逻辑，
//! 布局/控件工厂抽到 gui 模块，选项声明与构建装配分离。
mod gui;
use fltk::{
    app, button::{Button, CheckButton}, draw,
    enums::{Color, Font, FrameType},
    frame::Frame, group::{Pack, PackType},
    prelude::*,
    table::{TableContext, TableRow}, text::{TextBuffer, TextDisplay},
    window::Window,
};
use gui::{field, choice, checkrow, header_of, section, COL, CONTENT_W, MARGIN, WIN_H, WIN_W};
use std::sync::{Arc, Mutex};
use tiwi::core::builder;
use tiwi::core::manifest::{FileMap, Project};

type Rows = Arc<Mutex<Vec<Vec<String>>>>;

/// 表格行数据（全局承载：draw_cell 闭包需零捕获以满足 HRTB 泛型签名）
static FILE_ROWS: std::sync::OnceLock<Rows> = std::sync::OnceLock::new();

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.len() > 1 && args[1] == "--build" {
        headless(&args);
        return;
    }
    run_gui();
}

// ---------------- CLI（无头构建，CI/自动化用） ----------------

fn headless(args: &[String]) {
    let proj_path = match args.get(2) {
        Some(p) => p,
        None => {
            eprintln!("[E_ARGS] 用法: tiwi --build <project.tiwi.json> [--out <dir>]");
            std::process::exit(2);
        }
    };
    let text = std::fs::read_to_string(proj_path)
        .unwrap_or_else(|e| { eprintln!("[E_LOAD] {e}"); std::process::exit(10); });
    let proj = Project::from_json(&text)
        .unwrap_or_else(|e| { eprintln!("[E_LOAD] {e}"); std::process::exit(10); });
    let proj_dir = std::path::Path::new(proj_path).parent()
        .map(|p| p.to_path_buf()).unwrap_or_else(|| ".".into());
    let out_ov = args.windows(2)
        .find(|w| w[0] == "--out")
        .map(|w| std::path::PathBuf::from(&w[1]));
    match builder::build(&proj, &proj_dir, out_ov.as_deref()) {
        Ok(r) => { println!("[ok] 产物: {}", r.setup_exe.display()); std::process::exit(0); }
        Err(e) => { eprintln!("[E_BUILD] {e}"); std::process::exit(1); }
    }
}

// ---------------- GUI（FLTK 薄壳） ----------------

fn run_gui() {
    let mut app = app::App::default().with_scheme(app::Scheme::Gleam);
    let mut win = Window::new(90, 60, WIN_W, WIN_H, "tiwi — tie 安装程序制作器");
    win.make_resizable(true);
    win.size_range(760, 660, 2048, 1600);

    let rows: Rows = Arc::new(Mutex::new(Vec::new()));
    let _ = FILE_ROWS.set(rows.clone());

    // 主列：所有控件收进同一纵向 Pack（col.end() 之前创建）
    let mut col = Pack::default().with_size(WIN_W - MARGIN * 2, WIN_H - MARGIN * 2).with_pos(MARGIN, MARGIN);
    col.set_type(PackType::Vertical);
    col.set_spacing(6);

    // ---- 区块 1：应用信息 ----
    section(&mut col, "应用信息");
    let mut row_info = Pack::default().with_size(CONTENT_W, 116);
    row_info.set_type(PackType::Horizontal);
    row_info.set_spacing(12);
    let mut col_left = Pack::default().with_size(COL, 116);
    col_left.set_type(PackType::Vertical);
    col_left.set_spacing(4);
    let i_name = field(&mut col_left, COL, "应用名称", "hello-app");
    let i_id = field(&mut col_left, COL, "应用 ID", "org.tie.hello");
    let i_ver = field(&mut col_left, COL, "版本", "1.0.0");
    let i_pub = field(&mut col_left, COL, "发布者", "");
    let mut col_right = Pack::default().with_size(COL, 116);
    col_right.set_type(PackType::Vertical);
    col_right.set_spacing(4);
    let i_preset = choice(&mut col_right, COL, "预设", &["bare", "trm-bundled", "trm-detect", "runtime-toolchain"]);
    let i_format = choice(&mut col_right, COL, "负荷", &["binary", "tieir", "llvmir"]);
    let i_compile = choice(&mut col_right, COL, "编译策略", &["target", "build"]);
    let i_opt = field(&mut col_right, COL, "优化级别", "-O2");
    row_info.add(&col_left);
    row_info.add(&col_right);
    col.add(&row_info);

    // ---- 区块 2：构建配置 ----
    section(&mut col, "构建配置");
    let mut row_build = Pack::default().with_size(CONTENT_W, 116);
    row_build.set_type(PackType::Horizontal);
    row_build.set_spacing(12);
    let mut col_b1 = Pack::default().with_size(COL, 116);
    col_b1.set_type(PackType::Vertical);
    col_b1.set_spacing(4);
    let i_out = field(&mut col_b1, COL, "输出目录", "out");
    let i_install = field(&mut col_b1, COL, "安装目录(留空=默认)", "");
    let i_trmreq = field(&mut col_b1, COL, "TRM 版本要求", "^0");
    let mut col_b2 = Pack::default().with_size(COL, 116);
    col_b2.set_type(PackType::Vertical);
    col_b2.set_spacing(4);
    let i_trmsrc = field(&mut col_b2, COL, "TRM 目录(bundle)", "");
    let cb_bundle = checkrow(&mut col_b2, COL, "捆绑 TRM", false, "随包携带 TRM 运行时");
    let i_uninstall = field(&mut col_b2, COL, "卸载命令(可选)", "");
    row_build.add(&col_b1);
    row_build.add(&col_b2);
    col.add(&row_build);

    // ---- 区块 3：文件清单 ----
    section(&mut col, "文件清单");
    let mut table = TableRow::default().with_size(CONTENT_W, 200);
    table.set_cols(3);
    table.set_col_header(false);
    table.set_col_width(0, 330);
    table.set_col_width(1, 330);
    table.set_col_width(2, 110);
    table.draw_cell(|_t, ctx, row, col, x, y, w, h| match ctx {
        TableContext::Cell => {
            let header = row == 0;
            let text = match FILE_ROWS.get() {
                Some(rows) => {
                    let guard = rows.lock().unwrap();
                    if header {
                        header_of(col).to_string()
                    } else if (row as usize) - 1 < guard.len() {
                        guard[(row as usize) - 1][col as usize].clone()
                    } else {
                        String::new()
                    }
                }
                None => String::new(),
            };
            if header {
                let c = Color::from_rgb(216, 228, 240);
                draw::set_draw_color(c);
                draw::draw_box(FrameType::DownBox, x, y, w, h, c);
                draw::set_font(Font::HelveticaBold, 13);
                draw::set_draw_color(Color::DarkBlue);
            } else {
                draw::set_draw_color(Color::White);
                draw::draw_box(FrameType::DownBox, x, y, w, h, Color::White);
                draw::set_draw_color(Color::Black);
            }
            draw::draw_text(&text, x + 6, y + h - 9);
        }
        _ => {}
    });
    table.set_rows(rows.lock().unwrap().len() as i32 + 1);

    // ---- 按钮行 ----
    let mut btn_row = Pack::default().with_size(CONTENT_W, 30);
    btn_row.set_type(PackType::Horizontal);
    btn_row.set_spacing(8);
    let mut btn_add = Button::default().with_size(130, 28).with_label("添加文件行");
    let mut btn_del = Button::default().with_size(130, 28).with_label("删除末行");
    let mut spacer = Frame::default().with_size(CONTENT_W - 130 - 130 - 200 - 24, 28);
    spacer.set_frame(FrameType::NoBox);
    let mut btn_build = Button::default().with_size(200, 28).with_label("构建安装包");
    btn_row.add(&btn_add);
    btn_row.add(&btn_del);
    btn_row.add(&spacer);
    btn_row.add(&btn_build);
    col.add(&btn_row);

    // ---- 区块 4：构建日志 ----
    section(&mut col, "构建日志");
    let mut log = TextDisplay::default().with_size(CONTENT_W, 140);
    let mut buf = TextBuffer::default();
    log.set_buffer(buf.clone());
    log.set_text_color(Color::Black);

    // ---- 状态栏 ----
    let mut status = Frame::default().with_size(CONTENT_W, 22);
    status.set_align(fltk::enums::Align::Left | fltk::enums::Align::Inside);
    status.set_label_color(Color::DarkBlue);
    status.set_label("就绪 — 输出目录: out");

    col.end();
    win.end();

    let (tx, rx) = app::channel::<String>();
    let mut buf_build = buf.clone();

    let mut table_a = table.clone();
    btn_add.set_callback(move |_| {
        if let Some(rows) = FILE_ROWS.get() {
            rows.lock().unwrap().push(vec![String::new(), String::new(), "false".into()]);
            table_a.set_rows(rows.lock().unwrap().len() as i32 + 1);
        }
    });

    let mut table_d = table.clone();
    btn_del.set_callback(move |_| {
        if let Some(rows) = FILE_ROWS.get() {
            let mut lock = rows.lock().unwrap();
            if !lock.is_empty() {
                lock.pop();
                table_d.set_rows(lock.len() as i32 + 1);
            }
        }
    });

    btn_build.set_callback({
        let tx = tx.clone();
        move |_| {
            let mut p = Project::default();
            p.app.name = if i_name.value().is_empty() { "app".into() } else { i_name.value() };
            p.app.id = i_id.value();
            p.app.version = if i_ver.value().is_empty() { "1.0.0".into() } else { i_ver.value() };
            p.app.publisher = i_pub.value();
            p.preset = i_preset.text(i_preset.value()).unwrap_or_default();
            p.payload.format = i_format.text(i_format.value()).unwrap_or_default();
            p.payload.compile = i_compile.text(i_compile.value()).unwrap_or_default();
            p.payload.optimize = i_opt.value();
            p.trm.require = if i_trmreq.value().is_empty() { "^0".into() } else { i_trmreq.value() };
            p.trm.bundle = cb_bundle.is_checked();
            p.uninstall.cmd = i_uninstall.value();
            p.build.out_dir = if i_out.value().is_empty() { "out".into() } else { i_out.value() };
            p.build.install_dir = i_install.value();
            p.build.trm_source = i_trmsrc.value();
            if let Some(ro) = FILE_ROWS.get() {
                let lock = ro.lock().unwrap();
                for r in lock.iter() {
                    if r[0].is_empty() { continue; }
                    p.files.push(FileMap {
                        from: r[0].clone(),
                        to: if r[1].is_empty() { r[0].clone() } else { r[1].clone() },
                        exec: r[2].trim().eq_ignore_ascii_case("true"),
                    });
                }
            }
            let proj_dir = std::path::PathBuf::from(".");
            let out_s = p.build.out_dir.clone();
            let label = format!("[build] {} {} preset={} format={} bundle={}", p.app.name, p.app.version, p.preset, p.payload.format, p.trm.bundle);
            buf_build.append(&format!("{label}\n"));
            let tx2 = tx.clone();
            std::thread::spawn(move || {
                let (msg, st) = match builder::build(&p, &proj_dir, None) {
                    Ok(r) => (format!("[ok] 产物: {}\n", r.setup_exe.display()), format!("输出目录: {}", out_s)),
                    Err(e) => (format!("[E_BUILD] {e}\n"), String::from("构建失败")),
                };
                tx2.send(format!("{msg}\u{1}ST={st}"));
            });
        }
    });

    let mut status_s = status.clone();
    win.show();
    while app.wait() {
        if let Some(m) = rx.recv() {
            if let Some((text, st)) = m.split_once("\u{1}ST=") {
                buf.append(text);
                status_s.set_label(st);
            } else {
                buf.append(&m);
            }
        }
    }
}