//! HWPX 견적서 → RawTable (P0-5 에서 검증한 방식).
//!
//! 핵심: **`cellAddr` 로 격자를 복원한다.** 셀을 차례로 세면 병합된 자리만큼 어긋난다.

use std::io::Read;
use std::path::Path;

use quick_xml::events::{BytesStart, BytesText, Event};
use quick_xml::Reader;

use crate::error::{AppError, AppResult};
use crate::quote::model::{RawCell, RawTable};

fn local(name: &str) -> &str {
    name.rsplit(':').next().unwrap_or("")
}

fn attr(e: &BytesStart, key: &str) -> Option<String> {
    e.attributes().flatten().find(|a| a.key.as_ref() == key).map(|a| a.value.to_string())
}

/// quick-xml 0.42 는 str 기반이라 `BytesText` 에 `unescape` 가 없다.
fn xml_text(t: &BytesText) -> String {
    let raw = t.xml_content(quick_xml::XmlVersion::Implicit1_0);
    quick_xml::escape::unescape(&raw).map(|c| c.to_string()).unwrap_or_else(|_| raw.to_string())
}

#[derive(Debug, Default, Clone)]
struct Cell {
    col: usize,
    row: usize,
    col_span: usize,
    text: String,
}

pub fn read(path: &Path) -> AppResult<Vec<RawTable>> {
    let f = std::fs::File::open(path)?;
    let mut zip = zip::ZipArchive::new(f).map_err(|e| {
        AppError::new("HWPX_NOT_ZIP", "한글 문서를 열지 못했습니다. 파일이 손상되었을 수 있습니다. 품목을 직접 입력할 수 있습니다.").detail(e.to_string())
    })?;

    let sections: Vec<String> = zip
        .file_names()
        .filter(|n| n.starts_with("Contents/section") && n.ends_with(".xml"))
        .map(|s| s.to_string())
        .collect();
    if sections.is_empty() {
        return Err(AppError::new("HWPX_NO_SECTION", "한글 문서 안에서 본문을 찾지 못했습니다. 품목을 직접 입력할 수 있습니다."));
    }

    let mut tables = Vec::new();
    let mut sorted = sections;
    sorted.sort();
    for name in sorted {
        let mut xml = String::new();
        zip.by_name(&name)
            .map_err(|e| AppError::new("HWPX_READ", "한글 문서의 본문을 읽지 못했습니다. 품목을 직접 입력할 수 있습니다.").detail(e.to_string()))?
            .read_to_string(&mut xml)?;
        tables.extend(parse_section(&xml, tables.len()));
    }

    if tables.is_empty() {
        return Err(AppError::new("HWPX_NO_TABLE", "한글 문서 안에서 표를 찾지 못했습니다. 품목을 직접 입력할 수 있습니다."));
    }
    Ok(tables)
}

/// 한 section XML 안의 표들을 뽑는다.
pub fn parse_section(xml: &str, table_offset: usize) -> Vec<RawTable> {
    let mut r = Reader::from_str(xml);
    let mut out: Vec<RawTable> = Vec::new();

    let mut depth = 0usize;
    let mut cells: Vec<Cell> = Vec::new();
    let mut cur: Option<Cell> = None;
    let mut in_t = false;
    // 표 밖 글자 (합계금액 칸이 표 밖에 있는 양식을 위해)
    let mut loose: Vec<String> = Vec::new();

    loop {
        let ev = r.read_event();
        match ev {
            Ok(Event::Start(ref e)) | Ok(Event::Empty(ref e)) => match local(e.name().as_ref()) {
                "tbl" => {
                    depth += 1;
                    if depth == 1 {
                        cells.clear();
                    }
                }
                "tc" if depth == 1 => cur = Some(Cell { col_span: 1, ..Default::default() }),
                "cellAddr" if depth == 1 => {
                    if let Some(c) = cur.as_mut() {
                        c.col = attr(e, "colAddr").and_then(|v| v.parse().ok()).unwrap_or(0);
                        c.row = attr(e, "rowAddr").and_then(|v| v.parse().ok()).unwrap_or(0);
                    }
                }
                "cellSpan" if depth == 1 => {
                    if let Some(c) = cur.as_mut() {
                        c.col_span = attr(e, "colSpan").and_then(|v| v.parse().ok()).unwrap_or(1);
                    }
                }
                "t" => in_t = true,
                _ => {}
            },
            Ok(Event::Text(ref t)) if in_t => {
                let s = xml_text(t);
                match cur.as_mut() {
                    Some(c) => c.text.push_str(&s),
                    None if depth == 0 => loose.push(s),
                    None => {}
                }
            }
            Ok(Event::End(ref e)) => match local(e.name().as_ref()) {
                "t" => in_t = false,
                "tc" if depth == 1 => {
                    if let Some(c) = cur.take() {
                        cells.push(c);
                    }
                }
                "tbl" => {
                    if depth == 1 {
                        out.push(build_table(&cells, table_offset + out.len()));
                    }
                    depth = depth.saturating_sub(1);
                }
                _ => {}
            },
            Ok(Event::Eof) => break,
            Err(_) => break,
            _ => {}
        }
    }

    // 표 밖 글자는 모든 표에 참고로 붙인다
    if !loose.is_empty() {
        for t in out.iter_mut() {
            t.loose_text = loose.iter().map(|s| RawCell::plain(s.clone())).collect();
        }
    }
    out
}

/// `cellAddr` 로 격자를 만든다. 표 제목은 첫 행 글자에서 가져온다.
fn build_table(cells: &[Cell], table_no: usize) -> RawTable {
    let width = cells.iter().map(|c| c.col + c.col_span.max(1)).max().unwrap_or(0);
    let height = cells.iter().map(|c| c.row + 1).max().unwrap_or(0);

    let mut rows: Vec<Vec<RawCell>> = (0..height)
        .map(|r| {
            (0..width)
                .map(|c| RawCell::new("", format!("표{} {}행 {}열", table_no + 1, r + 1, c + 1)))
                .collect()
        })
        .collect();

    for c in cells {
        if c.row < height && c.col < width {
            rows[c.row][c.col].text = c.text.trim().to_string();
        }
    }

    // 한글 문서는 제목도 표 안에 있다 (P0-5 실측)
    let title = rows
        .first()
        .map(|r| r.iter().map(|c| c.text.as_str()).collect::<Vec<_>>().join(" "))
        .unwrap_or_default();

    RawTable { title, rows, loose_text: Vec::new() }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 병합이 섞인 표를 cellAddr 로 복원한다. 차례로 세면 어긋나는 모양이다.
    #[test]
    fn cell_addr_rebuilds_grid_with_merges() {
        let xml = r#"
<hs:sec xmlns:hp="x"><hp:tbl>
  <hp:tr>
    <hp:tc><hp:cellAddr colAddr="0" rowAddr="0"/><hp:cellSpan colSpan="3" rowSpan="1"/><hp:p><hp:run><hp:t>견 적 서</hp:t></hp:run></hp:p></hp:tc>
  </hp:tr>
  <hp:tr>
    <hp:tc><hp:cellAddr colAddr="0" rowAddr="1"/><hp:cellSpan colSpan="1" rowSpan="1"/><hp:p><hp:run><hp:t>품명</hp:t></hp:run></hp:p></hp:tc>
    <hp:tc><hp:cellAddr colAddr="1" rowAddr="1"/><hp:cellSpan colSpan="1" rowSpan="1"/><hp:p><hp:run><hp:t>수량</hp:t></hp:run></hp:p></hp:tc>
    <hp:tc><hp:cellAddr colAddr="2" rowAddr="1"/><hp:cellSpan colSpan="1" rowSpan="1"/><hp:p><hp:run><hp:t>공급가액</hp:t></hp:run></hp:p></hp:tc>
  </hp:tr>
  <hp:tr>
    <hp:tc><hp:cellAddr colAddr="0" rowAddr="2"/><hp:p><hp:run><hp:t>프로보테크닉 교구</hp:t></hp:run></hp:p></hp:tc>
    <hp:tc><hp:cellAddr colAddr="1" rowAddr="2"/><hp:p><hp:run><hp:t>10</hp:t></hp:run></hp:p></hp:tc>
    <hp:tc><hp:cellAddr colAddr="2" rowAddr="2"/><hp:p><hp:run><hp:t>765,000</hp:t></hp:run></hp:p></hp:tc>
  </hp:tr>
</hp:tbl></hs:sec>"#;
        let tables = parse_section(xml, 0);
        assert_eq!(tables.len(), 1);
        let t = &tables[0];
        assert!(t.title.contains("견 적 서"));
        assert_eq!(t.get(1, 0), "품명");
        assert_eq!(t.get(1, 2), "공급가액");
        assert_eq!(t.get(2, 0), "프로보테크닉 교구");
        assert_eq!(t.get(2, 2), "765,000");
        assert_eq!(t.cell(2, 0).unwrap().cell_ref, "표1 3행 1열");
    }

    #[test]
    fn two_tables_are_separate() {
        let xml = r#"
<hs:sec xmlns:hp="x">
<hp:tbl><hp:tr><hp:tc><hp:cellAddr colAddr="0" rowAddr="0"/><hp:p><hp:run><hp:t>견적서</hp:t></hp:run></hp:p></hp:tc></hp:tr></hp:tbl>
<hp:tbl><hp:tr><hp:tc><hp:cellAddr colAddr="0" rowAddr="0"/><hp:p><hp:run><hp:t>납품서</hp:t></hp:run></hp:p></hp:tc></hp:tr></hp:tbl>
</hs:sec>"#;
        let tables = parse_section(xml, 0);
        assert_eq!(tables.len(), 2);
        assert!(tables[0].title.contains("견적서"));
        assert!(tables[1].title.contains("납품서"));
    }
}
