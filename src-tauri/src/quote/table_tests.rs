//! 공통 표 해석 단위시험. 격자를 손으로 만들어 규칙만 본다(파일 형식과 무관).

use super::*;
use crate::quote::model::{RawCell, RawTable};

fn t(rows: &[&[&str]]) -> RawTable {
    RawTable {
        title: String::new(),
        rows: rows
            .iter()
            .enumerate()
            .map(|(r, row)| {
                row.iter()
                    .enumerate()
                    .map(|(c, s)| RawCell::new(*s, format!("R{r}C{c}")))
                    .collect()
            })
            .collect(),
        loose_text: Vec::new(),
    }
}

#[test]
fn finds_header_anywhere() {
    // 머리글이 5행에 있어도 찾아야 한다
    let table = t(&[
        &["견 적 서", "", "", ""],
        &["", "", "", ""],
        &["합계금액", "444,000", "", ""],
        &["", "", "", ""],
        &["", "", "", ""],
        &["품명", "규격", "수량", "단가", "공급가액"],
        &["바둑교재(상상바둑)", "권", "37", "12,000", "444,000"],
    ]);
    let q = interpret(&table);
    assert_eq!(q.items.len(), 1);
    assert_eq!(q.items[0].display_name, "바둑교재(상상바둑)");
    assert_eq!(q.items[0].qty, Some(37));
    assert_eq!(q.items[0].unit_price, Some(12_000));
    assert_eq!(q.items[0].amount, Some(444_000));
    assert_eq!(q.item_sum, 444_000);
    assert_eq!(q.grand_total, Some(444_000));
    assert_eq!(q.compare_total(), 444_000);
}

#[test]
fn raw_values_are_kept_alongside_parsed() {
    let table = t(&[
        &["품명", "규격", "수량", "단가", "금액"],
        &["교재A", "권", "10개", "76,500원", "765,000"],
    ]);
    let q = interpret(&table);
    let i = &q.items[0];
    assert_eq!(i.raw_qty, "10개");
    assert_eq!(i.raw_unit_price, "76,500원");
    assert_eq!(i.qty, Some(10));
    assert_eq!(i.unit_price, Some(76_500));
    assert_eq!(i.display_name, i.raw_name, "처음에는 표시명 = 원문");
    assert_eq!(i.cell_ref, "R1C0");
}

#[test]
fn empty_and_total_rows_are_not_items() {
    let table = t(&[
        &["품명", "규격", "수량", "단가", "공급가액"],
        &["교재A", "권", "10", "1,000", "10,000"],
        &["", "", "", "", ""],
        &["", "", "", "", "0"],
        &["합계", "", "10", "", "10,000"],
    ]);
    let q = interpret(&table);
    assert_eq!(q.items.len(), 1, "빈 행과 합계 행은 품목이 아니다");
    assert_eq!(q.item_sum, 10_000);
    assert_eq!(q.supply_total, Some(10_000), "합계 행 값을 공급가액으로 본다");
}

/// 설계안 14장 9번의 예시 그대로
#[test]
fn discount_excluded_from_count_but_counted_in_total() {
    let table = t(&[
        &["품명", "규격", "수량", "단가", "금액"],
        &["교재A", "", "", "", "100,000"],
        &["교재B", "", "", "", "200,000"],
        &["할인", "", "", "", "-30,000"],
        &["합계", "", "", "", "270,000"],
    ]);
    let q = interpret(&table);
    assert_eq!(q.items.len(), 3);
    let items: Vec<&ParsedItem> = q.items.iter().filter(|i| i.kind == RowKind::Item).collect();
    assert_eq!(items.len(), 2, "N종은 2종");
    let adj = q.items.iter().find(|i| i.kind == RowKind::Adjustment).unwrap();
    assert_eq!(adj.amount, Some(-30_000), "원문 부호 그대로 보존");
    assert_eq!(adj.sign_effect, Some(SignEffect::AsWritten));
    assert_eq!(q.item_sum, 300_000);
    assert_eq!(q.adjustment_sum, -30_000);
    assert_eq!(q.computed_total, 270_000);
}

/// 할인이 양수로 적혀 있어도 **원본을 고치지 않고** 부호 효과만 정한다
#[test]
fn positive_discount_is_detected_by_reconciliation() {
    let table = t(&[
        &["품명", "규격", "수량", "단가", "금액"],
        &["교재A", "", "", "", "100,000"],
        &["교재B", "", "", "", "200,000"],
        &["할인", "", "", "", "30,000"],
        &["합계", "", "", "", "270,000"],
    ]);
    let q = interpret(&table);
    let adj = q.items.iter().find(|i| i.kind == RowKind::Adjustment).unwrap();
    assert_eq!(adj.amount, Some(30_000), "원문은 양수 그대로 남는다");
    assert_eq!(adj.raw_amount, "30,000");
    assert_eq!(adj.sign_effect, Some(SignEffect::Subtract), "빼는 값으로 판별");
    assert_eq!(q.adjustment_sum, -30_000);
    assert_eq!(q.computed_total, 270_000);
}

/// 어느 쪽으로도 맞지 않으면 추정하지 않고 경고한다
#[test]
fn unknown_discount_sign_warns_and_contributes_zero() {
    let table = t(&[
        &["품명", "규격", "수량", "단가", "금액"],
        &["교재A", "", "", "", "100,000"],
        &["할인", "", "", "", "30,000"],
        // 총액이 어느 쪽과도 안 맞는다
        &["합계", "", "", "", "999,999"],
    ]);
    let q = interpret(&table);
    let adj = q.items.iter().find(|i| i.kind == RowKind::Adjustment).unwrap();
    assert_eq!(adj.sign_effect, Some(SignEffect::Unknown));
    assert_eq!(q.adjustment_sum, 0, "모를 때는 계산에 넣지 않는다");
    assert!(q.warnings.iter().any(|w| w.code == "ADJUSTMENT_SIGN_UNKNOWN"));
}

/// 설계안 14장 10번의 예시 그대로
#[test]
fn zero_amount_row_is_kept_but_excluded() {
    let table = t(&[
        &["품명", "규격", "수량", "단가", "금액"],
        &["교재A", "", "", "", "100,000"],
        &["교재B", "", "", "", "200,000"],
        &["사은품 노트", "", "1", "0", "0"],
    ]);
    let q = interpret(&table);
    assert_eq!(q.items.len(), 3, "0원 행도 화면에 보여야 하므로 남긴다");
    let zero = q.items.iter().find(|i| i.kind == RowKind::Zero).unwrap();
    assert_eq!(zero.display_name, "사은품 노트");
    assert_eq!(q.items.iter().filter(|i| i.kind == RowKind::Item).count(), 2, "N종은 2종");
    assert_eq!(q.item_sum, 300_000, "0원 행은 총액에 영향이 없다");
    assert_eq!(q.computed_total, 300_000);
}

/// 판별 기준은 '사은품' 이라는 낱말이 아니라 **금액이 0인지**다
#[test]
fn zero_detection_is_by_amount_not_by_word() {
    let table = t(&[
        &["품명", "규격", "수량", "단가", "금액"],
        &["사은품 세트", "", "1", "5,000", "5,000"],
        &["일반 교재", "", "1", "0", "0"],
    ]);
    let q = interpret(&table);
    assert_eq!(q.items[0].kind, RowKind::Item, "사은품이어도 금액이 있으면 품목");
    assert_eq!(q.items[1].kind, RowKind::Zero, "이름과 무관하게 금액 0이면 0원 행");
}

#[test]
fn qty_price_mismatch_warns_without_fixing() {
    let table = t(&[
        &["품명", "규격", "수량", "단가", "금액"],
        &["교재A", "", "10", "1,000", "9,000"],
    ]);
    let q = interpret(&table);
    assert_eq!(q.items[0].amount, Some(9_000), "값을 고치지 않는다");
    assert!(q.items[0].warnings.iter().any(|w| w.code == "QTY_PRICE_MISMATCH"));
}

#[test]
fn small_rounding_is_info_not_warning() {
    let table = t(&[
        &["품명", "규격", "수량", "단가", "금액"],
        &["교재A", "", "3", "3,333", "9,990"],
    ]);
    let q = interpret(&table);
    let w = &q.items[0].warnings[0];
    assert_eq!(w.code, "QTY_PRICE_MISMATCH");
    assert_eq!(w.severity, crate::domain::Severity::Info, "10원 미만은 절사로 본다");
}

/// '단위' 는 규격의 동의어, `품명/규격` 합친 열도 받는다 (설계안 B9)
#[test]
fn spec_synonyms() {
    let table = t(&[
        &["순번", "품 명/규 격", "단위", "수량", "단가", "공급가액"],
        &["1", "방과후 기초Yap! 상", "권", "3", "10,000", "30,000"],
    ]);
    let q = interpret(&table);
    assert_eq!(q.items.len(), 1);
    assert_eq!(q.items[0].display_name, "방과후 기초Yap! 상");
    assert_eq!(q.items[0].spec, "권");
    assert_eq!(q.items[0].amount, Some(30_000));
}

/// 병합 때문에 값이 머리글 칸이 아니라 옆 칸에 있는 경우 (P0-2 실측)
#[test]
fn merged_columns_find_value_in_range() {
    // '단가' 머리글은 3열, 값은 5열에 있다. '공급가액' 머리글은 6열, 값은 7열.
    let table = t(&[
        &["품명", "규격", "수량", "단가", "", "", "공급가액", "", "비고"],
        &["교재A", "권", "37", "", "", "12,000", "", "444,000", "면세"],
    ]);
    let q = interpret(&table);
    assert_eq!(q.items[0].unit_price, Some(12_000));
    assert_eq!(q.items[0].amount, Some(444_000));
}

#[test]
fn picks_quote_table_by_title() {
    let mut a = t(&[&["품명", "금액"], &["교재", "1,000"]]);
    a.title = "No.    견  적  서".into();
    let mut b = t(&[&["품명", "금액"], &["교재", "1,000"]]);
    b.title = "No.    납  품  서".into();

    let (i, note, warn) = pick_table(&[a.clone(), b.clone()]);
    assert_eq!(i, 0);
    assert!(note.contains("견적서"));
    assert!(warn.is_none());

    // 차례가 바뀌어도 제목으로 고른다
    let (i, _, _) = pick_table(&[b, a]);
    assert_eq!(i, 1);
}

#[test]
fn single_table_needs_no_choice() {
    let a = t(&[&["품명", "금액"]]);
    let (i, _, warn) = pick_table(&[a]);
    assert_eq!(i, 0);
    assert!(warn.is_none());
}

#[test]
fn no_header_gives_warning_not_garbage() {
    let table = t(&[&["아무", "글자"], &["더", "있음"]]);
    let q = interpret(&table);
    assert!(q.items.is_empty());
    assert!(q.warnings.iter().any(|w| w.code == "HEADER_AMBIGUOUS"));
}

#[test]
fn vendor_name_is_read_for_reference() {
    let table = t(&[
        &["상호", "(주)가나상사", "성명", "홍길동"],
        &["품명", "규격", "수량", "단가", "금액"],
        &["교재", "권", "1", "1,000", "1,000"],
    ]);
    let q = interpret(&table);
    assert_eq!(q.vendor_name_in_doc, "(주)가나상사");
}

#[test]
fn no_total_falls_back_to_item_sum_with_warning() {
    let table = t(&[
        &["순번", "품명", "수량", "단가", "공급가액"],
        &["1", "교재A", "3", "10,000", "30,000"],
        &["2", "교재B", "1", "8,000", "8,000"],
    ]);
    let q = interpret(&table);
    assert_eq!(q.compare_basis, CompareBasis::ComputedTotal);
    assert_eq!(q.compare_total(), 38_000);
    assert!(q.warnings.iter().any(|w| w.code == "TOTAL_NOT_FOUND"));
}

/// 상호 칸이 비어 있으면 **옆 이름표를 끌어오지 않는다** (실제 앱에서 '등록번호' 가 상호로 보였다)
#[test]
fn empty_vendor_field_stays_empty() {
    let table = t(&[
        &["등록번호", "", "상호(법인명)", "", "성명", "홍길동"],
        &["품명", "규격", "수량", "단가", "금액"],
        &["교재", "권", "1", "1,000", "1,000"],
    ]);
    let q = interpret(&table);
    assert_eq!(q.vendor_name_in_doc, "", "빈 칸이면 빈 값이어야 한다");
}

#[test]
fn vendor_name_is_read_when_present() {
    let table = t(&[
        &["상호", "(주)가나상사", "성명", "홍길동"],
        &["품명", "규격", "수량", "단가", "금액"],
        &["교재", "권", "1", "1,000", "1,000"],
    ]);
    assert_eq!(interpret(&table).vendor_name_in_doc, "(주)가나상사");
}
