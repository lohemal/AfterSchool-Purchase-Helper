//! XLSX 를 "셀 단위 사양" 으로 읽어 내는 도구.
//!
//! 왜 이렇게 만드는가:
//! - 스타일 **번호**는 파일마다 다르므로, 번호가 아니라 번호가 가리키는 **실제 값**으로 풀어서 비교한다.
//! - 색은 `rgb` · `indexed` · `theme` · `auto` 로 제각각 적히므로 **RGB 문자열로 정규화**한다.
//!   (서로 다른 생성기가 만든 파일은 XML 이 절대 같을 수 없다. 같아야 하는 것은 *뜻*이다.)
//! - 테두리는 네 방향을 **따로** 본다. 한쪽 선이 빠진 것을 놓치면 안 된다.
//!
//! 이 도구는 본 프로그램의 "생성 후 다시 읽어 검증" 단계에 그대로 쓸 수 있다.

use quick_xml::events::{BytesStart, BytesText, Event};
use quick_xml::Reader;
use std::collections::HashMap;
use std::io::Read;
use std::path::Path;

/// quick-xml 0.42 는 str 기반이라 `BytesText` 에 `unescape` 가 없다.
fn xml_text(t: &BytesText) -> String {
    let raw = t.xml_content(quick_xml::XmlVersion::Implicit1_0);
    quick_xml::escape::unescape(&raw)
        .map(|c| c.to_string())
        .unwrap_or_else(|_| raw.to_string())
}

fn attr(e: &BytesStart, key: &str) -> Option<String> {
    e.attributes()
        .flatten()
        .find(|a| a.key.as_ref() == key)
        .map(|a| a.value.to_string())
}

fn tag_of(name: &str) -> String {
    name.rsplit(':').next().unwrap_or("").to_string()
}

fn local(e: &BytesStart) -> String {
    e.name().as_ref().rsplit(':').next().unwrap_or("").to_string()
}

/// 엑셀 옛 색 번호표(0~63). 64 는 "자동"(시스템 글자색).
const INDEXED: [&str; 64] = [
    "000000", "FFFFFF", "FF0000", "00FF00", "0000FF", "FFFF00", "FF00FF", "00FFFF", "000000",
    "FFFFFF", "FF0000", "00FF00", "0000FF", "FFFF00", "FF00FF", "00FFFF", "800000", "008000",
    "000080", "808000", "800080", "008080", "C0C0C0", "808080", "9999FF", "993366", "FFFFCC",
    "CCFFFF", "660066", "FF8080", "0066CC", "CCCCFF", "000080", "FF00FF", "FFFF00", "00FFFF",
    "800080", "800000", "008080", "0000FF", "00CCFF", "CCFFFF", "CCFFCC", "FFFF99", "99CCFF",
    "FF99CC", "CC99FF", "FFCC99", "3366FF", "33CCCC", "99CC00", "FFCC00", "FF9900", "FF6600",
    "666699", "969696", "003366", "339966", "003300", "333300", "993300", "993366", "333399",
    "333333",
];

/// 색 요소 → 정규화된 표현. `auto` 와 indexed 64 는 둘 다 "auto" 로 본다(같은 뜻이다).
fn color_of(e: &BytesStart) -> String {
    if attr(e, "auto").as_deref() == Some("1") {
        return "auto".into();
    }
    if let Some(rgb) = attr(e, "rgb") {
        // AARRGGBB → RRGGBB. 앞 2글자(알파)만 뗀다.
        // trim_start_matches("FF") 를 쓰면 "FFFFFFFF"(흰색)가 통째로 사라진다.
        return if rgb.len() == 8 { rgb[2..].to_string() } else { rgb };
    }
    if let Some(i) = attr(e, "indexed") {
        let n: usize = i.parse().unwrap_or(64);
        return if n >= 64 { "auto".into() } else { INDEXED[n].to_string() };
    }
    if let Some(t) = attr(e, "theme") {
        // theme 1 = dk1 = 기본 글자색. 색을 아예 안 적은 것(auto)과 뜻이 같다.
        // (샘플은 색을 안 적고, rust_xlsxwriter 는 theme="1" 을 적는다 — 화면에 보이는 결과는 같다.)
        if t == "1" {
            return "auto".into();
        }
        return format!("theme:{t}");
    }
    "auto".into()
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CellValue {
    Text(String),
    Number(String),
    Empty,
}

/// 한 방향 테두리: (선 모양, 색). 선이 없으면 ("", "").
pub type Side = (String, String);

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CellSpec {
    pub reference: String,
    pub value: CellValue,
    pub num_fmt: String,
    pub font_name: String,
    pub font_size: String,
    pub bold: bool,
    pub font_color: String,
    pub fill: String, // "none" 또는 "solid:RRGGBB"
    pub align_h: String,
    pub align_v: String,
    pub wrap: bool,
    pub left: Side,
    pub right: Side,
    pub top: Side,
    pub bottom: Side,
    pub formula: Option<String>,
}

impl CellSpec {
    /// 사람이 읽는 한 줄 (차이를 보고할 때 쓴다)
    pub fn describe(&self) -> String {
        format!(
            "값={:?} 형식={} 글꼴={}/{}{} 글자색={} 채움={} 정렬={}/{}{} 테두리 L{:?} R{:?} T{:?} B{:?}",
            self.value,
            self.num_fmt,
            self.font_name,
            self.font_size,
            if self.bold { "/굵게" } else { "" },
            self.font_color,
            self.fill,
            self.align_h,
            self.align_v,
            if self.wrap { "/줄바꿈" } else { "" },
            self.left,
            self.right,
            self.top,
            self.bottom
        )
    }
}

#[derive(Debug, Clone)]
pub struct SheetSpec {
    pub name: String,
    pub dimension: String,
    pub cells: Vec<CellSpec>,
    pub merged: Vec<String>,
    pub col_widths: Vec<(u32, u32, String)>,
    pub hidden_rows: Vec<u32>,
    pub hidden_cols: Vec<u32>,
}

impl SheetSpec {
    pub fn cell(&self, reference: &str) -> Option<&CellSpec> {
        self.cells.iter().find(|c| c.reference == reference)
    }
}

#[derive(Debug, Clone)]
pub struct BookSpec {
    pub sheet_names: Vec<String>,
    pub sheets: Vec<SheetSpec>,
    pub has_calc_chain: bool,
}

// ---------------------------------------------------------------- styles

#[derive(Default, Clone)]
struct Font {
    name: String,
    size: String,
    bold: bool,
    color: String,
}

#[derive(Default, Clone)]
struct Border {
    left: Side,
    right: Side,
    top: Side,
    bottom: Side,
}

#[derive(Default, Clone)]
struct Xf {
    num_fmt_id: String,
    font_id: usize,
    fill_id: usize,
    border_id: usize,
    align_h: String,
    align_v: String,
    wrap: bool,
}

struct Styles {
    xfs: Vec<Xf>,
    fonts: Vec<Font>,
    fills: Vec<String>,
    borders: Vec<Border>,
    custom_fmts: HashMap<String, String>,
}

fn builtin_num_fmt(id: &str) -> String {
    match id {
        "0" => "General",
        "1" => "0",
        "2" => "0.00",
        "3" => "#,##0",
        "4" => "#,##0.00",
        "9" => "0%",
        "10" => "0.00%",
        other => return format!("builtin:{other}"),
    }
    .to_string()
}

impl Styles {
    fn parse(xml: &str) -> Styles {
        let mut r = Reader::from_str(xml);
        let (mut xfs, mut fonts, mut fills, mut borders) = (vec![], vec![], vec![], vec![]);
        let mut custom = HashMap::new();

        // 어느 구역 안인지 (styles.xml 은 cellStyleXfs 와 cellXfs 가 둘 다 <xf> 다)
        let mut sect = String::new();
        let mut cur_xf: Option<Xf> = None;
        let mut cur_font = Font::default();
        let mut cur_border = Border::default();
        let mut cur_side = String::new();
        let mut cur_fill = String::from("none");
        let mut in_pattern = false;

        loop {
            let ev = r.read_event();
            let (e, is_empty) = match &ev {
                Ok(Event::Start(e)) => (e.clone(), false),
                Ok(Event::Empty(e)) => (e.clone(), true),
                Ok(Event::End(e)) => {
                    match tag_of(e.name().as_ref()).as_str() {
                        t @ ("cellXfs" | "cellStyleXfs" | "fonts" | "fills" | "borders") => {
                            if sect == t {
                                sect.clear();
                            }
                        }
                        "xf" if sect == "cellXfs" => {
                            if let Some(x) = cur_xf.take() {
                                xfs.push(x);
                            }
                        }
                        "font" if sect == "fonts" => fonts.push(std::mem::take(&mut cur_font)),
                        "border" if sect == "borders" => {
                            borders.push(std::mem::take(&mut cur_border))
                        }
                        "fill" if sect == "fills" => {
                            fills.push(std::mem::replace(&mut cur_fill, "none".into()))
                        }
                        "patternFill" => in_pattern = false,
                        "left" | "right" | "top" | "bottom" => cur_side.clear(),
                        _ => {}
                    }
                    continue;
                }
                Ok(Event::Eof) => break,
                Err(_) => break,
                _ => continue,
            };

            match local(&e).as_str() {
                t @ ("cellXfs" | "cellStyleXfs" | "fonts" | "fills" | "borders") => {
                    sect = t.to_string()
                }
                "numFmt" => {
                    if let (Some(id), Some(code)) = (attr(&e, "numFmtId"), attr(&e, "formatCode")) {
                        custom.insert(id, code);
                    }
                }
                // --- cellXfs ---
                "xf" if sect == "cellXfs" => {
                    let x = Xf {
                        num_fmt_id: attr(&e, "numFmtId").unwrap_or_else(|| "0".into()),
                        font_id: attr(&e, "fontId").and_then(|v| v.parse().ok()).unwrap_or(0),
                        fill_id: attr(&e, "fillId").and_then(|v| v.parse().ok()).unwrap_or(0),
                        border_id: attr(&e, "borderId").and_then(|v| v.parse().ok()).unwrap_or(0),
                        ..Default::default()
                    };
                    // 자기닫음 <xf/> 는 End 가 오지 않는다 → 여기서 바로 넣어야 번호가 안 밀린다.
                    if is_empty {
                        xfs.push(x);
                    } else {
                        cur_xf = Some(x);
                    }
                }
                "alignment" => {
                    if let Some(x) = cur_xf.as_mut() {
                        x.align_h = attr(&e, "horizontal").unwrap_or_default();
                        x.align_v = attr(&e, "vertical").unwrap_or_default();
                        x.wrap = attr(&e, "wrapText").as_deref() == Some("1");
                    }
                }
                // --- fonts ---
                "font" if sect == "fonts" => {
                    cur_font = Font { color: "auto".into(), ..Default::default() };
                    if is_empty {
                        fonts.push(std::mem::take(&mut cur_font));
                    }
                }
                "name" | "rFont" if sect == "fonts" => {
                    cur_font.name = attr(&e, "val").unwrap_or_default()
                }
                "sz" if sect == "fonts" => cur_font.size = attr(&e, "val").unwrap_or_default(),
                "b" if sect == "fonts" => cur_font.bold = attr(&e, "val").as_deref() != Some("0"),
                "color" if sect == "fonts" => cur_font.color = color_of(&e),
                // --- fills ---
                "fill" if sect == "fills" => {
                    cur_fill = "none".into();
                    if is_empty {
                        fills.push(std::mem::replace(&mut cur_fill, "none".into()));
                    }
                }
                "patternFill" => {
                    in_pattern = true;
                    let p = attr(&e, "patternType").unwrap_or_else(|| "none".into());
                    cur_fill = if p == "none" { "none".into() } else { format!("{p}:") };
                }
                "fgColor" if in_pattern => {
                    if cur_fill != "none" {
                        cur_fill = format!("{}{}", cur_fill, color_of(&e));
                    }
                }
                // --- borders ---
                "border" if sect == "borders" => {
                    cur_border = Border::default();
                    if is_empty {
                        borders.push(std::mem::take(&mut cur_border));
                    }
                }
                s @ ("left" | "right" | "top" | "bottom") if sect == "borders" => {
                    cur_side = s.to_string();
                    let style = attr(&e, "style").unwrap_or_default();
                    let side: Side = (style, "auto".into());
                    match s {
                        "left" => cur_border.left = side,
                        "right" => cur_border.right = side,
                        "top" => cur_border.top = side,
                        _ => cur_border.bottom = side,
                    }
                }
                "color" if sect == "borders" && !cur_side.is_empty() => {
                    let c = color_of(&e);
                    match cur_side.as_str() {
                        "left" => cur_border.left.1 = c,
                        "right" => cur_border.right.1 = c,
                        "top" => cur_border.top.1 = c,
                        _ => cur_border.bottom.1 = c,
                    }
                }
                _ => {}
            }
        }

        Styles { xfs, fonts, fills, borders, custom_fmts: custom }
    }

    fn resolve(&self, s: Option<&str>) -> (String, Font, String, Border, String, String, bool) {
        let idx: usize = s.unwrap_or("0").parse().unwrap_or(0);
        let xf = self.xfs.get(idx).cloned().unwrap_or_default();
        let fmt = self
            .custom_fmts
            .get(&xf.num_fmt_id)
            .cloned()
            .unwrap_or_else(|| builtin_num_fmt(&xf.num_fmt_id));
        let font = self.fonts.get(xf.font_id).cloned().unwrap_or_default();
        let fill = self.fills.get(xf.fill_id).cloned().unwrap_or_else(|| "none".into());
        let border = self.borders.get(xf.border_id).cloned().unwrap_or_default();
        (fmt, font, fill, border, xf.align_h, xf.align_v, xf.wrap)
    }
}

// ---------------------------------------------------------------- reading

fn read_part(zip: &mut zip::ZipArchive<std::fs::File>, name: &str) -> Option<String> {
    let mut f = zip.by_name(name).ok()?;
    let mut s = String::new();
    f.read_to_string(&mut s).ok()?;
    Some(s)
}

pub fn read_book(path: &Path) -> BookSpec {
    let file =
        std::fs::File::open(path).unwrap_or_else(|e| panic!("열 수 없다 {}: {e}", path.display()));
    let mut zip = zip::ZipArchive::new(file).expect("xlsx 는 zip 이다");

    let has_calc_chain = zip.file_names().any(|n| n.contains("calcChain"));
    let wb = read_part(&mut zip, "xl/workbook.xml").expect("workbook.xml");
    let styles = Styles::parse(&read_part(&mut zip, "xl/styles.xml").unwrap_or_default());

    // 공유 문자열
    let mut shared: Vec<String> = Vec::new();
    if let Some(ss) = read_part(&mut zip, "xl/sharedStrings.xml") {
        let mut r = Reader::from_str(&ss);
        let mut cur = String::new();
        let mut in_si = false;
        loop {
            match r.read_event() {
                Ok(Event::Start(e)) => {
                    if tag_of(e.name().as_ref()) == "si" {
                        in_si = true;
                        cur.clear();
                    }
                }
                Ok(Event::Text(t)) if in_si => cur.push_str(&xml_text(&t)),
                Ok(Event::End(e)) => {
                    if tag_of(e.name().as_ref()) == "si" {
                        in_si = false;
                        shared.push(std::mem::take(&mut cur));
                    }
                }
                Ok(Event::Eof) | Err(_) => break,
                _ => {}
            }
        }
    }

    // 시트 이름
    let mut sheet_names = Vec::new();
    {
        let mut r = Reader::from_str(&wb);
        loop {
            match r.read_event() {
                Ok(Event::Start(e)) | Ok(Event::Empty(e)) => {
                    if local(&e) == "sheet" {
                        if let Some(n) = attr(&e, "name") {
                            sheet_names.push(n);
                        }
                    }
                }
                Ok(Event::Eof) | Err(_) => break,
                _ => {}
            }
        }
    }

    let mut sheets = Vec::new();
    for (i, name) in sheet_names.iter().enumerate() {
        let part = format!("xl/worksheets/sheet{}.xml", i + 1);
        if let Some(xml) = read_part(&mut zip, &part) {
            sheets.push(parse_sheet(name, &xml, &shared, &styles));
        }
    }

    BookSpec { sheet_names, sheets, has_calc_chain }
}

fn parse_sheet(name: &str, xml: &str, shared: &[String], styles: &Styles) -> SheetSpec {
    let mut r = Reader::from_str(xml);
    let (mut cells, mut merged, mut col_widths) = (vec![], vec![], vec![]);
    let (mut hidden_rows, mut hidden_cols) = (vec![], vec![]);
    let mut dimension = String::new();

    let mut cur_ref = String::new();
    let mut cur_t = String::new();
    let mut cur_s: Option<String> = None;
    let mut cur_v = String::new();
    let mut cur_f: Option<String> = None;
    let (mut in_v, mut in_f, mut in_is) = (false, false, false);

    loop {
        let ev = r.read_event();
        let (e, _is_empty) = match &ev {
            Ok(Event::Start(e)) => (e.clone(), false),
            Ok(Event::Empty(e)) => (e.clone(), true),
            Ok(Event::Text(txt)) => {
                let s = xml_text(txt);
                if in_v || in_is {
                    cur_v.push_str(&s);
                } else if in_f {
                    cur_f = Some(cur_f.take().unwrap_or_default() + &s);
                }
                continue;
            }
            Ok(Event::End(e)) => {
                match tag_of(e.name().as_ref()).as_str() {
                    "v" => in_v = false,
                    "f" => in_f = false,
                    "is" => in_is = false,
                    "c" => {
                        let value = if cur_v.is_empty() {
                            CellValue::Empty
                        } else if cur_t == "s" {
                            let i: usize = cur_v.trim().parse().unwrap_or(usize::MAX);
                            CellValue::Text(shared.get(i).cloned().unwrap_or_default())
                        } else if cur_t == "inlineStr" || cur_t == "str" {
                            CellValue::Text(cur_v.clone())
                        } else {
                            CellValue::Number(cur_v.clone())
                        };
                        if value != CellValue::Empty || cur_f.is_some() {
                            let (fmt, font, fill, b, ah, av, wrap) =
                                styles.resolve(cur_s.as_deref());
                            cells.push(CellSpec {
                                reference: cur_ref.clone(),
                                value,
                                num_fmt: fmt,
                                font_name: font.name,
                                font_size: font.size,
                                bold: font.bold,
                                font_color: font.color,
                                fill,
                                align_h: ah,
                                align_v: av,
                                wrap,
                                left: b.left,
                                right: b.right,
                                top: b.top,
                                bottom: b.bottom,
                                formula: cur_f.clone(),
                            });
                        }
                    }
                    _ => {}
                }
                continue;
            }
            Ok(Event::Eof) => break,
            Err(_) => break,
            _ => continue,
        };

        match local(&e).as_str() {
            "dimension" => dimension = attr(&e, "ref").unwrap_or_default(),
            "mergeCell" => {
                if let Some(x) = attr(&e, "ref") {
                    merged.push(x)
                }
            }
            "col" => {
                let min = attr(&e, "min").and_then(|v| v.parse().ok()).unwrap_or(0);
                let max = attr(&e, "max").and_then(|v| v.parse().ok()).unwrap_or(0);
                if attr(&e, "hidden").as_deref() == Some("1") {
                    hidden_cols.push(min);
                }
                col_widths.push((min, max, attr(&e, "width").unwrap_or_default()));
            }
            "row" => {
                if attr(&e, "hidden").as_deref() == Some("1") {
                    if let Some(n) = attr(&e, "r").and_then(|v| v.parse().ok()) {
                        hidden_rows.push(n);
                    }
                }
            }
            "c" => {
                cur_ref = attr(&e, "r").unwrap_or_default();
                cur_t = attr(&e, "t").unwrap_or_default();
                cur_s = attr(&e, "s");
                cur_v.clear();
                cur_f = None;
            }
            "v" => in_v = true,
            "f" => in_f = true,
            "is" => in_is = true,
            _ => {}
        }
    }

    SheetSpec { name: name.into(), dimension, cells, merged, col_widths, hidden_rows, hidden_cols }
}
