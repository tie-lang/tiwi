//! GUI 布局助手（模块化：区/行/控件工厂 + 常量），供 bin/tiwi.rs 适配层复用。
//! 界面上的每个字段行 = section/field/choice/checkrow 一行声明，新增选项只加一行调用。
use fltk::{
    button::CheckButton,
    enums::{Align, Color, FrameType},
    frame::Frame,
    group::{Pack, PackType},
    input::Input,
    menu::Choice,
    prelude::*,
};

pub const COL: i32 = 416;
pub const LABEL: i32 = 150;
pub const WIN_W: i32 = 880;
pub const WIN_H: i32 = 786;
pub const MARGIN: i32 = 12;
pub const CONTENT_W: i32 = WIN_W - MARGIN * 2 - 24;

/// 分区标题条
pub fn section(parent: &mut Pack, title: &str) {
    let mut f = Frame::default().with_size(CONTENT_W, 22);
    f.set_frame(FrameType::DownFrame);
    f.set_label(&title);
    f.set_label_color(Color::DarkBlue);
    f.set_label_size(13);
    parent.add(&f);
}

/// 标签 + 输入框 行（纵向 Pack 内一行）
pub fn field(parent: &mut Pack, w: i32, label: &str, dflt: &str) -> Input {
    let mut row = Pack::default().with_size(w, 26);
    row.set_type(PackType::Horizontal);
    row.set_spacing(8);
    row.begin();
    let mut l = Frame::default().with_size(LABEL, 24).with_label(label);
    l.set_align(Align::Left | Align::Inside);
    let mut i = Input::default().with_size(w - LABEL - 8, 26);
    i.set_value(&dflt);
    row.end();
    parent.add(&row);
    i
}

/// 标签 + 下拉框 行
pub fn choice(parent: &mut Pack, w: i32, label: &str, items: &[&str]) -> Choice {
    let mut row = Pack::default().with_size(w, 26);
    row.set_type(PackType::Horizontal);
    row.set_spacing(8);
    row.begin();
    let mut l = Frame::default().with_size(LABEL, 24).with_label(label);
    l.set_align(Align::Left | Align::Inside);
    let mut c = Choice::default().with_size(w - LABEL - 8, 26);
    for it in items {
        c.add_choice(it);
    }
    c.set_value(0);
    row.end();
    parent.add(&row);
    c
}

/// 标签 + 复选框 行
pub fn checkrow(parent: &mut Pack, w: i32, label: &str, dflt: bool, tip: &str) -> CheckButton {
    let mut row = Pack::default().with_size(w, 26);
    row.set_type(PackType::Horizontal);
    row.set_spacing(8);
    row.begin();
    let mut l = Frame::default().with_size(LABEL, 24).with_label(label);
    l.set_align(Align::Left | Align::Inside);
    let mut c = CheckButton::default().with_size(w - LABEL - 8, 26).with_label(tip);
    if dflt { c.set_value(true); }
    row.end();
    parent.add(&row);
    c
}

pub fn header_of(col: i32) -> &'static str {
    match col {
        0 => "源文件(相对工程)",
        1 => "目标(安装树)",
        _ => "可执行",
    }
}