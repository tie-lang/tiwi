//! tiwi 入口：`--build <proj> [--out <dir>]` 走无头 CLI；无参则启动 FLTK GUI。
//! GUI 是 core 的薄适配层（六边形架构的"适配器"）：UI 不持有任何构建逻辑。
use fltk::{
    app, button::Button, draw, enums::{Align, Color, FrameType}, frame::Frame, group::Pack,
    input::Input, prelude::*, table::{Table, TableContext, TableRow}, text::{TextBuffer, TextDisplay},
    menu::Choice, window::Window,
};
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
    let mut win = Window::new(90, 60, 820, 680, "tiwi — tie 安装程序制作器");
    win.make_resizable(true);

    let rows: Rows = Arc::new(Mutex::new(vec![vec![
        "sample/hello.txt".to_string(),
        "app/hello.txt".to_string(),
        "false".to_string(),
    ]]));
    let _ = FILE_ROWS.set(rows.clone());

    let mut col = Pack::default().with_size(800, 660).with_pos(10, 10);
    col.set_type(fltk::group::PackType::Vertical);
    col.set_spacing(6);

    let i_name = field(&mut col, "应用名称", "hello-app");
    let i_id = field(&mut col, "应用 ID", "org.tie.hello");
    let i_ver = field(&mut col, "版本", "1.0.0");
    let i_preset = choice(&mut col, "预设", &["bare", "trm-bundled", "trm-detect", "runtime-toolchain"]);
    let i_format = choice(&mut col, "负荷", &["binary", "tieir", "llvmir"]);
    let i_out = field(&mut col, "输出目录", "out");
    let i_install = field(&mut col, "安装目录(留空=默认)", "");
    let i_trmsrc = field(&mut col, "TRM 目录(bundle)", "");

    let mut table = TableRow::default().with_size(780, 170);
    table.set_cols(3);
    table.set_col_header(false);
    table.set_col_width(0, 330);
    table.set_col_width(1, 330);
    table.set_col_width(2, 110);
    table.draw_cell(|_t, ctx, row, col, x, y, w, h| match ctx {
        TableContext::Cell => {
            let text = match FILE_ROWS.get() {
                Some(rows) => {
                    let guard = rows.lock().unwrap();
                    if row == 0 {
                        header_of(col).to_string()
                    } else if (row as usize) - 1 < guard.len() {
                        guard[(row as usize) - 1][col as usize].clone()
                    } else {
                        String::new()
                    }
                }
                None => String::new(),
            };
            draw::set_draw_color(Color::Black);
            draw::draw_text(&text, x + 4, y + h - 8);
            draw::draw_box(FrameType::DownBox, x, y, w, h, Color::White);
        }
        _ => {}
    });    table.set_rows(rows.lock().unwrap().len() as i32 + 1);

    let mut btn_row = Pack::default().with_size(800, 30);
    btn_row.set_type(fltk::group::PackType::Horizontal);
    btn_row.set_spacing(8);
    let mut btn_add = Button::default().with_size(120, 28).with_label("添加文件行");
    let mut btn_build = Button::default().with_size(170, 28).with_label("构建安装包");
    btn_row.end();

    let mut log = TextDisplay::default().with_size(800, 160);
    let mut buf = TextBuffer::default();
    log.set_buffer(buf.clone());
    log.set_text_color(Color::Black);

    col.end();

    win.end();

    let (tx, rx) = app::channel::<String>();

    let mut buf_build = buf.clone();
    btn_add.set_callback(move |_| {
        if let Some(rows) = FILE_ROWS.get() {
            rows.lock().unwrap().push(vec![String::new(), String::new(), "false".into()]);
            table.set_rows(rows.lock().unwrap().len() as i32 + 1);
        }
    });

    btn_build.set_callback({
        let tx = tx.clone();
        move |_| {
            let mut p = Project::default();
            p.app.name = if i_name.value().is_empty() { "app".into() } else { i_name.value() };
            p.app.id = i_id.value();
            p.app.version = if i_ver.value().is_empty() { "1.0.0".into() } else { i_ver.value() };
            p.preset = i_preset.text(i_preset.value()).unwrap_or_default();
            p.payload.format = i_format.text(i_format.value()).unwrap_or_default();
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
            buf_build.append(&format!("[build] {} {} preset={} format={}\n", p.app.name, p.app.version, p.preset, p.payload.format));
            let tx2 = tx.clone();
            std::thread::spawn(move || {
                let msg = match builder::build(&p, &proj_dir, None) {
                    Ok(r) => format!("[ok] 产物: {}\n", r.setup_exe.display()),
                    Err(e) => format!("[E_BUILD] {e}\n"),
                };
                tx2.send(msg);
            });
        }
    });

    win.show();
    while app.wait() {
        if let Some(m) = rx.recv() {
            buf.append(&m);
        }
    }
}

fn field(parent: &mut Pack, label: &str, dflt: &str) -> Input {
    let mut row = Pack::default().with_size(800, 26);
    row.set_type(fltk::group::PackType::Horizontal);
    row.set_spacing(8);
    row.begin();
    let mut l = Frame::default().with_size(150, 24).with_label(label);
    l.set_align(fltk::enums::Align::Left | fltk::enums::Align::Inside);
    let mut i = Input::default().with_size(620, 26);
    i.set_value(&dflt);
    row.end();
    parent.add(&row);
    i
}

fn choice(parent: &mut Pack, label: &str, items: &[&str]) -> Choice {
    let mut row = Pack::default().with_size(800, 26);
    row.set_type(fltk::group::PackType::Horizontal);
    row.set_spacing(8);
    row.begin();
    let mut l = Frame::default().with_size(150, 24).with_label(label);
    l.set_align(fltk::enums::Align::Left | fltk::enums::Align::Inside);
    let mut c = Choice::default().with_size(260, 26);
    for it in items {
        c.add_choice(it);
    }
    c.set_value(0);
    row.end();
    parent.add(&row);
    c
}

fn header_of(col: i32) -> &'static str {
    match col {
        0 => "源文件(相对工程)",
        1 => "目标(安装树)",
        _ => "可执行",
    }
}