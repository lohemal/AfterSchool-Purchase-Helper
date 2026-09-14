//! P0-1  에듀파인 XLSX 재현
//!
//! 시험 내용: `rust_xlsxwriter` 로 만든 품의 파일이 실제 업로드에 쓴 샘플과
//! **셀 단위로 같은가**. 시트명·머리글·19행 값·자료형·표시형식·서식까지 본다.
//! 샘플: test/fixtures-local/9월 교재 재료 품의서(1).xlsx  (읽기만 한다)
//! 산출: test/out/p0-1-생성.xlsx

use probe::xlsx_spec::{read_book, CellValue};
use probe::{fixture, out};
use rust_xlsxwriter::{Format, FormatAlign, FormatBorder, Workbook};

const SAMPLE: &str = "9월 교재 재료 품의서(1).xlsx";

/// 샘플에 실제로 들어 있는 19행. (내용, 예상단가)
const ROWS: &[(&str, u32)] = &[
    ("코딩부 창의코딩놀이 엔트리 외 1종", 224_000),
    ("컴퓨터부 내친구 한글왕 2022 외 5종", 332_000),
    ("드론항공과학교구세트 1종", 600_000),
    ("통합과학부 사마귀 외 4종", 742_500),
    ("로봇과학부 프로보테크닉 교구 외 1종", 720_000),
    ("로봇과학부 익스트림에디션 키트 외 1종", 90_000),
    ("한자급수부 8급 외 2종", 58_000),
    ("영어회화부 ENGLISH BUS STARTER 외 2종", 446_400),
    ("만화애니메이션부 만들기/패브릭복주머니 외 3종", 462_000),
    ("토탈공예미니어처부 보관함 세트 외 2종", 690_000),
    ("종이접기부 태극무늬꽂이 외 4종", 180_000),
    ("체스부 매직체스스쿨 1종", 196_000),
    ("체스부 체스는내친구 1종", 72_000),
    ("키즈쿠킹부 초코칩쿠키 외 10종", 2_191_000),
    ("바둑부 바둑교재 1종", 372_000),
    ("창의독서부 메이킹북 1종", 90_000),
    ("역사탐구부 훌륭한위인이야기 외 1종", 345_000),
    ("주산암산부 방과후 기초Yap! 상 외 3종", 40_000),
    ("우쿠렐레부 막쳤는데 잘쳐지는 우쿨렐레 찐초보 1종", 36_000),
];

/// 생성기 사양 (설계안 10장). 시트명은 상수 한 곳에만 둔다.
const SHEET_NAME: &str = "품목내역";
const HEADERS: [&str; 4] = ["내용", "규격", "수량", "예상단가"];
const SPEC_TEXT: &str = "식";

fn build(path: &std::path::Path) {
    let mut wb = Workbook::new();
    let ws = wb.add_worksheet();
    ws.set_name(SHEET_NAME).unwrap();

    // 샘플의 실제 모습: 머리글은 **검정 바탕 + 흰 굵은 글씨 + 빨강 가는 테두리**,
    // 본문은 자동색 가는 테두리. 첫 머리글 칸에만 왼쪽 선이 있다(샘플 그대로).
    let head_base = || {
        Format::new()
            .set_bold()
            .set_font_color(0xFFFFFF)
            .set_background_color(0x000000)
            .set_align(FormatAlign::Center)
            .set_align(FormatAlign::VerticalCenter)
            .set_text_wrap()
            .set_font_name("Dotum")
            .set_font_size(9)
            .set_border_right(FormatBorder::Thin)
            .set_border_top(FormatBorder::Thin)
            .set_border_bottom(FormatBorder::Thin)
            .set_border_color(0xFF0000)
    };
    let head_first = head_base().set_border_left(FormatBorder::Thin);
    let head_rest = head_base();

    let text = Format::new()
        .set_border(FormatBorder::Thin)
        .set_align(FormatAlign::Left)
        .set_align(FormatAlign::VerticalCenter)
        .set_text_wrap()
        .set_font_name("Dotum")
        .set_font_size(9);
    let num = Format::new()
        .set_border(FormatBorder::Thin)
        .set_align(FormatAlign::Right)
        .set_align(FormatAlign::VerticalCenter)
        .set_text_wrap()
        .set_num_format_index(3) // #,##0
        .set_font_name("Dotum")
        .set_font_size(9);

    for (c, h) in HEADERS.iter().enumerate() {
        let f = if c == 0 { &head_first } else { &head_rest };
        ws.write_string_with_format(0, c as u16, *h, f).unwrap();
    }
    for (i, (content, amount)) in ROWS.iter().enumerate() {
        let r = (i + 1) as u32;
        ws.write_string_with_format(r, 0, *content, &text).unwrap();
        ws.write_string_with_format(r, 1, SPEC_TEXT, &text).unwrap();
        ws.write_number_with_format(r, 2, 1.0, &num).unwrap();
        ws.write_number_with_format(r, 3, *amount as f64, &num).unwrap();
    }
    // 열 너비: 샘플은 44.5 / 12.25 를 저장한다. rust_xlsxwriter 는 문자너비를 픽셀로 바꿨다가
    // 되돌리므로 저장값이 픽셀 단위(약 0.14 문자)로 양자화되어 **그 두 값에 정확히 닿지 못한다**
    // (p0_1b_colwidth 에서 80~330픽셀을 전수 확인했다). 가장 가까운 값을 쓴다.
    ws.set_column_width_pixels(0, 311).unwrap(); // → 44.42578125 (샘플 44.5, 차이 0.07 문자)
    ws.set_column_width_pixels(1, 86).unwrap(); // → 12.28515625 (샘플 12.25, 차이 0.04 문자)
    ws.set_column_width_pixels(2, 86).unwrap();
    ws.set_column_width_pixels(3, 86).unwrap();

    wb.save(path).unwrap();
}

#[test]
fn p0_1_edufine_roundtrip() {
    let sample_path = fixture(SAMPLE);
    let made_path = out("p0-1-생성.xlsx");
    build(&made_path);

    let sample = read_book(&sample_path);
    let made = read_book(&made_path);

    // --- 1. 시트 ---
    assert_eq!(sample.sheet_names, vec![SHEET_NAME.to_string()], "샘플 시트 구성");
    assert_eq!(made.sheet_names, sample.sheet_names, "생성 파일의 시트 이름·개수");

    let s = &sample.sheets[0];
    let m = &made.sheets[0];

    // --- 2. 구조 ---
    assert_eq!(s.merged.len(), 0, "샘플에 병합 셀이 없어야 한다");
    assert_eq!(m.merged.len(), 0, "생성 파일에 병합 셀이 없어야 한다");
    assert_eq!(s.hidden_rows.len(), 0);
    assert_eq!(m.hidden_rows.len(), 0);
    assert_eq!(s.hidden_cols.len(), 0);
    assert_eq!(m.hidden_cols.len(), 0);
    assert!(s.cells.iter().all(|c| c.formula.is_none()), "샘플에 수식이 없어야 한다");
    assert!(m.cells.iter().all(|c| c.formula.is_none()), "생성 파일에 수식이 없어야 한다");
    assert_eq!(s.dimension, "A1:D20", "샘플 범위");
    assert_eq!(m.dimension, s.dimension, "생성 파일 범위");

    // --- 3. 셀 단위 비교 (값·자료형·표시형식·글꼴·글자색·채움·정렬·네 방향 테두리 전부) ---
    assert_eq!(m.cells.len(), s.cells.len(), "셀 개수");
    let mut diffs = Vec::new();
    for (a, b) in s.cells.iter().zip(m.cells.iter()) {
        if a.reference != b.reference {
            diffs.push(format!("위치 {} vs {}", a.reference, b.reference));
            continue;
        }
        if a != b {
            diffs.push(format!("{}\n      샘플: {}\n      생성: {}", a.reference, a.describe(), b.describe()));
        }
    }
    assert!(diffs.is_empty(), "샘플과 다른 곳 {}군데:\n    {}", diffs.len(), diffs.join("\n    "));

    // --- 4. 값 자체가 사양대로인지 (샘플이 맞다고 믿지 않고 다시 확인) ---
    let find = |r: &str| m.cells.iter().find(|c| c.reference == r).unwrap();
    for (c, h) in HEADERS.iter().enumerate() {
        let cell = find(&format!("{}1", (b'A' + c as u8) as char));
        assert_eq!(cell.value, CellValue::Text(h.to_string()), "머리글 {}", h);
    }
    for (i, (content, amount)) in ROWS.iter().enumerate() {
        let r = i + 2;
        assert_eq!(find(&format!("A{r}")).value, CellValue::Text(content.to_string()));
        assert_eq!(find(&format!("B{r}")).value, CellValue::Text(SPEC_TEXT.into()));
        assert_eq!(find(&format!("C{r}")).value, CellValue::Number("1".into()), "C{r} 수량은 숫자 1");
        assert_eq!(
            find(&format!("D{r}")).value,
            CellValue::Number(amount.to_string()),
            "D{r} 예상단가는 숫자"
        );
        assert_eq!(find(&format!("C{r}")).num_fmt, "#,##0");
        assert_eq!(find(&format!("D{r}")).num_fmt, "#,##0");
    }

    // --- 5. 열 너비: 정확히 같을 수 없으므로 허용 오차로 본다 (알려진 차이) ---
    let width_of = |sp: &probe::xlsx_spec::SheetSpec, col: u32| -> f64 {
        sp.col_widths
            .iter()
            .find(|(min, max, _)| col >= *min && col <= *max)
            .and_then(|(_, _, w)| w.parse::<f64>().ok())
            .unwrap_or(0.0)
    };
    for col in 1..=4u32 {
        let (ws_, wm) = (width_of(s, col), width_of(m, col));
        let gap = (ws_ - wm).abs();
        assert!(
            gap < 0.15,
            "{}열 너비 차이가 크다: 샘플 {ws_} vs 생성 {wm}",
            (b'A' + col as u8 - 1) as char
        );
    }

    // --- 6. 합계 행이 없어야 한다 ---
    assert!(
        !m.cells.iter().any(|c| matches!(&c.value, CellValue::Text(t) if t.contains("합계"))),
        "합계 행을 만들면 안 된다"
    );

    println!("P0-1 통과: 셀 {}개 일치, 시트명 {:?}", m.cells.len(), m.name);
}

#[test]
fn p0_1_calamine_readback() {
    // 생성기가 아니라 '다시 읽어 검증하는 쪽'(설계안 10장 저장 절차)을 확인한다.
    // 시험은 병렬로 도므로 **파일을 시험마다 따로 쓴다**(같은 이름을 쓰면 쓰는 중에 읽어 깨진다).
    use calamine::{open_workbook, Data, Reader, Xlsx};
    let made_path = out("p0-1-재읽기.xlsx");
    build(&made_path);
    let mut wb: Xlsx<_> = open_workbook(&made_path).expect("calamine 으로 열기");
    assert_eq!(wb.sheet_names(), vec![SHEET_NAME.to_string()]);
    let range = wb.worksheet_range(SHEET_NAME).expect("품목내역 시트");
    assert_eq!(range.height(), 20, "머리글 1 + 자료 19");
    assert_eq!(range.width(), 4);

    let mut total = 0i64;
    for r in 1..20 {
        let qty = range.get_value((r, 2)).unwrap();
        let price = range.get_value((r, 3)).unwrap();
        assert!(matches!(qty, Data::Float(_) | Data::Int(_)), "수량은 숫자여야 한다: {qty:?}");
        let p = match price {
            Data::Float(f) => *f as i64,
            Data::Int(i) => *i,
            other => panic!("예상단가가 숫자가 아니다: {other:?}"),
        };
        total += p;
    }
    let expect: i64 = ROWS.iter().map(|(_, a)| *a as i64).sum();
    assert_eq!(total, expect, "다시 읽은 합계");
    println!("P0-1 재읽기 통과: 19행 합계 {total}");
}
