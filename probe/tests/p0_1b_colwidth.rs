//! P0-1 곁가지: 열 너비를 샘플과 **정확히** 같게 쓸 수 있는가?
//! 샘플은 width="44.5" 와 "12.25" 를 저장한다. rust_xlsxwriter 는 문자너비→픽셀→문자너비로
//! 되돌리므로 값이 밀린다. 도달 가능한 값들을 실제로 만들어 확인한다.

use probe::out;
use probe::xlsx_spec::read_book;
use rust_xlsxwriter::Workbook;

fn stored_width_for_pixels(px: u32) -> String {
    let path = out(&format!("p0-1b-px{px}.xlsx"));
    let mut wb = Workbook::new();
    let ws = wb.add_worksheet();
    ws.set_column_width_pixels(0, px).unwrap();
    ws.write_string(0, 0, "x").unwrap();
    wb.save(&path).unwrap();
    let b = read_book(&path);
    let w = b.sheets[0].col_widths.first().map(|c| c.2.clone()).unwrap_or_default();
    let _ = std::fs::remove_file(&path);
    w
}

fn stored_width_for_chars(ch: f64) -> String {
    let path = out(&format!("p0-1b-ch{}.xlsx", ch.to_string().replace('.', "_")));
    let mut wb = Workbook::new();
    let ws = wb.add_worksheet();
    ws.set_column_width(0, ch).unwrap();
    ws.write_string(0, 0, "x").unwrap();
    wb.save(&path).unwrap();
    let b = read_book(&path);
    let w = b.sheets[0].col_widths.first().map(|c| c.2.clone()).unwrap_or_default();
    let _ = std::fs::remove_file(&path);
    w
}

#[test]
fn p0_1b_column_width_reachable() {
    println!("--- set_column_width(문자수) 로 저장되는 값 ---");
    for ch in [12.25_f64, 12.0, 11.5, 44.5, 44.0, 43.5, 43.0] {
        println!("  요청 {ch:>6} → 저장 {}", stored_width_for_chars(ch));
    }
    println!("--- set_column_width_pixels(픽셀) 로 저장되는 값 ---");
    let mut hit_44_5 = None;
    let mut hit_12_25 = None;
    for px in 80..=330u32 {
        let w = stored_width_for_pixels(px);
        if w == "44.5" {
            hit_44_5 = Some(px);
        }
        if w == "12.25" {
            hit_12_25 = Some(px);
        }
    }
    println!("  44.5 를 만드는 픽셀값: {hit_44_5:?}");
    println!("  12.25 를 만드는 픽셀값: {hit_12_25:?}");
    println!("  (근방) px=310 → {} · px=311 → {}", stored_width_for_pixels(310), stored_width_for_pixels(311));
    println!("  (근방) px=85  → {} · px=86  → {}", stored_width_for_pixels(85), stored_width_for_pixels(86));
}
