//! OCR 신뢰 판단 시험. **가짜 확률을 만들지 않고, 값도 고치지 않는다** 는 것이 요지다.

use super::*;
use crate::quote::model::{RawCell, RawTable};

fn table_of(rows: &[&[&str]]) -> RawTable {
    RawTable {
        title: "견적서".into(),
        rows: rows
            .iter()
            .enumerate()
            .map(|(r, row)| {
                row.iter().enumerate().map(|(c, s)| RawCell::new(*s, format!("R{r}C{c}"))).collect()
            })
            .collect(),
        loose_text: Vec::new(),
    }
}

#[test]
fn ocr_never_gets_high_confidence() {
    // 모든 검증을 통과해도 사진에서 읽은 것은 medium 까지다
    let mut q = table::interpret(&table_of(&[
        &["품명", "규격", "수량", "단가", "금액"],
        &["교재A", "권", "3", "10,000", "30,000"],
    ]));
    apply(&mut q, Source::Ocr);
    assert_eq!(q.items[0].confidence, Confidence::Medium);
    assert_eq!(summarize(&q, Source::Ocr), QuoteTrust::NeedsCheck);
}

#[test]
fn structured_files_are_untouched() {
    // 합계 줄까지 있는 온전한 표 — 경고가 없어야 정상 이 나온다
    let before = table::interpret(&table_of(&[
        &["품명", "규격", "수량", "단가", "금액"],
        &["교재A", "권", "3", "10,000", "30,000"],
        &["합계", "", "", "", "30,000"],
    ]));
    let mut after = before.clone();
    apply(&mut after, Source::Structured);
    assert_eq!(after.warnings.len(), before.warnings.len(), "XLSX·HWP 는 손대지 않는다");
    assert_eq!(after.items[0].confidence, Confidence::High);
    assert_eq!(summarize(&after, Source::Structured), QuoteTrust::Ok);
}

/// 사용자가 든 예: 3 × 10,000 = 80,000 → **고치지 않고 검증 실패로 표시**
#[test]
fn amount_mismatch_is_reported_not_fixed() {
    let mut q = table::interpret(&table_of(&[
        &["품명", "규격", "수량", "단가", "금액"],
        &["교재A", "권", "3", "10,000", "80,000"],
    ]));
    apply(&mut q, Source::Ocr);

    assert_eq!(q.items[0].amount, Some(80_000), "값을 고치면 안 된다");
    assert_eq!(q.items[0].qty, Some(3));
    assert_eq!(q.items[0].unit_price, Some(10_000));
    assert_eq!(q.items[0].confidence, Confidence::Low);
    assert_eq!(summarize(&q, Source::Ocr), QuoteTrust::AmountFailed);
    assert!(q.warnings.iter().any(|w| w.code == "OCR_AMOUNT_CHECK"));
}

/// 수량×단가=금액 이 맞으면 숫자를 제대로 읽었다는 강한 근거다
#[test]
fn consistent_arithmetic_is_the_evidence() {
    let mut q = table::interpret(&table_of(&[
        &["품명", "규격", "수량", "단가", "금액"],
        &["교재A", "권", "3", "10,000", "30,000"],
        &["교재B", "권", "1", "8,000", "8,000"],
    ]));
    apply(&mut q, Source::Ocr);
    assert!(q.items.iter().all(|i| i.confidence == Confidence::Medium));
    assert!(!q.warnings.iter().any(|w| w.code == "OCR_AMOUNT_CHECK"));
}

#[test]
fn garbled_text_is_flagged() {
    // 한글 자모 낱자가 섞이면 깨진 것으로 본다 (OCR 이 흔히 만든다)
    let mut q = table::interpret(&table_of(&[
        &["품명", "규격", "수량", "단가", "금액"],
        &["ㅂ바방과후", "권", "3", "10,000", "30,000"],
    ]));
    apply(&mut q, Source::Ocr);
    assert_eq!(q.items[0].confidence, Confidence::Low);
    assert!(q.items[0].warnings.iter().any(|w| w.code == "OCR_GARBLED"));
    assert_eq!(summarize(&q, Source::Ocr), QuoteTrust::Uncertain);
}

#[test]
fn missing_amount_column_is_uncertain() {
    let mut q = table::interpret(&table_of(&[
        &["품명", "규격", "수량", "단가"],
        &["교재A", "권", "3", "10,000"],
    ]));
    apply(&mut q, Source::Ocr);
    assert_eq!(summarize(&q, Source::Ocr), QuoteTrust::Uncertain);
}

/// 표기 합계를 못 읽는 것은 사진에서 정상 경로다 (P0-6: `₩68,000` 이 깨졌다)
#[test]
fn missing_stated_total_falls_back_with_notice() {
    let mut q = table::interpret(&table_of(&[
        &["순번", "품명", "수량", "단가", "공급가액"],
        &["1", "교재A", "3", "10,000", "30,000"],
    ]));
    apply(&mut q, Source::Ocr);
    assert_eq!(q.computed_total, 30_000);
    assert!(q.warnings.iter().any(|w| w.code == "OCR_TOTAL_FALLBACK"));
    // 안내일 뿐 막지는 않는다
    assert_eq!(summarize(&q, Source::Ocr), QuoteTrust::NeedsCheck);
}

#[test]
fn pdf_text_is_trusted_more_than_ocr_but_still_checked() {
    let mut q = table::interpret(&table_of(&[
        &["품명", "규격", "수량", "단가", "금액"],
        &["교재A", "권", "3", "10,000", "30,000"],
        &["합계", "", "", "", "30,000"],
    ]));
    apply(&mut q, Source::PdfText);
    assert_eq!(q.items[0].confidence, Confidence::High, "좌표가 정확하면 값도 정확하다");
    assert_eq!(summarize(&q, Source::PdfText), QuoteTrust::Ok);
    assert!(!q.warnings.iter().any(|w| w.code == "OCR_USED"));
}

#[test]
fn trust_labels() {
    assert_eq!(QuoteTrust::Ok.label(), "정상");
    assert_eq!(QuoteTrust::NeedsCheck.label(), "확인 필요");
    assert_eq!(QuoteTrust::Uncertain.label(), "인식 불확실");
    assert_eq!(QuoteTrust::AmountFailed.label(), "금액 검증 실패");
}

#[test]
fn header_hits_uses_the_shared_dictionary() {
    let cells: Vec<String> =
        ["순번", "품 명/규 격", "단위", "수량", "단가", "공급가액", "OH", "비", "고"]
            .iter()
            .map(|s| s.to_string())
            .collect();
    // 세액('OH')과 비고('비'/'고')가 깨져도 6개는 맞는다 — P0-6 실측과 같다
    assert_eq!(header_hits(&cells), 6);
}

#[test]
fn garbled_detector_does_not_flag_normal_korean() {
    assert!(!looks_garbled("방과후 기초Yap! 상"));
    assert!(!looks_garbled("바둑교재(상상바둑)"));
    assert!(!looks_garbled("10급Yap!"));
    assert!(!looks_garbled(""));
    assert!(looks_garbled("ㅂ바방"));
}

/// 합계를 못 찾는 것은 사진이 아니어도 '확인 필요' 다 (읽기 문제가 아니라 자료 문제)
#[test]
fn missing_total_is_needs_check_even_for_structured() {
    let mut q = table::interpret(&table_of(&[
        &["품명", "규격", "수량", "단가", "금액"],
        &["교재A", "권", "3", "10,000", "30,000"],
    ]));
    apply(&mut q, Source::Structured);
    assert!(q.warnings.iter().any(|w| w.code == "TOTAL_NOT_FOUND"));
    assert_eq!(summarize(&q, Source::Structured), QuoteTrust::NeedsCheck);
}

/// 실제 주산암산 사진: `₩68,000` 이 `鬧8,000` 으로 읽혔다.
/// **68,000 으로 되돌리지 않고**, 그 합계를 못 읽은 것으로 다룬다.
#[test]
fn broken_stated_total_is_not_repaired_but_set_aside() {
    let mut q = table::interpret(&table_of(&[
        &["합계금액", "鬧8,000"],
        &["품명", "규격", "수량", "단가", "금액"],
        &["교재A", "권", "3", "10,000", "30,000"],
        &["교재B", "권", "1", "8,000", "8,000"],
    ]));
    assert_eq!(q.grand_total, Some(8_000), "읽은 값은 읽은 대로 남는다");
    apply(&mut q, Source::Ocr);

    assert_eq!(q.grand_total, Some(8_000), "값을 지우거나 고치지 않는다");
    assert_eq!(q.raw_grand_total, "鬧8,000", "원문도 남는다");
    assert_eq!(q.compare_basis, crate::domain::CompareBasis::ComputedTotal);
    assert_eq!(q.compare_total(), 38_000, "품목 합으로 비교한다");
    assert!(q.warnings.iter().any(|w| w.code == "OCR_TOTAL_UNREADABLE"));
    assert_eq!(summarize(&q, Source::Ocr), QuoteTrust::NeedsCheck, "사람이 총액을 봐야 한다");
}

/// 깨끗하게 읽힌 합계가 품목 합과 다르면 **고치지 않고 금액 검증 실패**
#[test]
fn clean_stated_total_that_disagrees_is_a_failure() {
    let mut q = table::interpret(&table_of(&[
        &["합계금액", "99,000"],
        &["품명", "규격", "수량", "단가", "금액"],
        &["교재A", "권", "3", "10,000", "30,000"],
    ]));
    apply(&mut q, Source::Ocr);
    assert_eq!(q.grand_total, Some(99_000));
    assert_eq!(q.compare_total(), 99_000, "비교 기준을 몰래 바꾸지 않는다");
    assert!(q.warnings.iter().any(|w| w.code == "OCR_TOTAL_MISMATCH"));
    assert_eq!(summarize(&q, Source::Ocr), QuoteTrust::AmountFailed);
}

#[test]
fn clean_stated_total_that_agrees_is_kept() {
    let mut q = table::interpret(&table_of(&[
        &["합계금액", "₩ 30,000"],
        &["품명", "규격", "수량", "단가", "금액"],
        &["교재A", "권", "3", "10,000", "30,000"],
    ]));
    apply(&mut q, Source::Ocr);
    assert_eq!(q.compare_basis, crate::domain::CompareBasis::Grand);
    assert_eq!(q.compare_total(), 30_000);
    assert!(!q.warnings.iter().any(|w| w.code == "OCR_TOTAL_UNREADABLE"));
    assert!(!q.warnings.iter().any(|w| w.code == "OCR_TOTAL_MISMATCH"));
}

/// XLSX·HWP 는 이 규칙을 타지 않는다 (P1 동작을 건드리지 않는다)
#[test]
fn structured_totals_are_never_set_aside() {
    let mut q = table::interpret(&table_of(&[
        &["합계금액", "鬧8,000"],
        &["품명", "규격", "수량", "단가", "금액"],
        &["교재A", "권", "3", "10,000", "30,000"],
    ]));
    apply(&mut q, Source::Structured);
    assert_eq!(q.compare_basis, crate::domain::CompareBasis::Grand);
    assert_eq!(q.compare_total(), 8_000);
    assert!(!q.warnings.iter().any(|w| w.code == "OCR_TOTAL_UNREADABLE"));
}

#[test]
fn broken_number_detector() {
    // 글자가 숫자에 붙은 것 — 못 읽은 것으로 본다
    assert!(number_looks_broken("鬧8,000"));
    assert!(number_looks_broken("68,0O0"));
    assert!(number_looks_broken("8,000鬧"));
    assert!(number_looks_broken("합계"));
    // 정상으로 읽힌 것
    assert!(!number_looks_broken("68,000"));
    assert!(!number_looks_broken("₩68,000"));
    assert!(!number_looks_broken("￦ 68,000"));
    assert!(!number_looks_broken("W68,000"));
    assert!(!number_looks_broken("68,000 원"));
    assert!(!number_looks_broken("합계금액 68,000"));
    assert!(!number_looks_broken("(30,000)"));
    // 금액 뒤의 `원정` 은 정상 표기다
    assert!(!number_looks_broken("8,000원정"));
}

/// 승인 규칙 — 표기 합계를 못 쓰는데 **품목 합도 검증되지 않으면** 정상으로 두지 않는다
#[test]
fn fallback_total_without_verifiable_items_is_uncertain() {
    // 단가가 없어 수량×단가=금액 을 맞춰 볼 수 없다
    let mut q = table::interpret(&table_of(&[
        &["합계금액", "鬧8,000"],
        &["품명", "규격", "수량", "단가", "금액"],
        &["교재A", "권", "3", "", "30,000"],
    ]));
    apply(&mut q, Source::Ocr);

    assert_eq!(q.compare_basis, crate::domain::CompareBasis::ComputedTotal);
    assert!(q.warnings.iter().any(|w| w.code == "OCR_TOTAL_UNREADABLE"));
    assert!(q.warnings.iter().any(|w| w.code == "OCR_TOTAL_UNVERIFIED"));
    assert_eq!(summarize(&q, Source::Ocr), QuoteTrust::Uncertain);
}

/// 합계를 아예 못 찾았을 때도 같은 기준을 쓴다
#[test]
fn missing_total_without_verifiable_items_is_uncertain() {
    let mut q = table::interpret(&table_of(&[
        &["순번", "품명", "수량", "단가", "공급가액"],
        &["1", "교재A", "", "", "30,000"],
    ]));
    apply(&mut q, Source::Ocr);
    assert!(q.warnings.iter().any(|w| w.code == "OCR_TOTAL_FALLBACK"));
    assert!(q.warnings.iter().any(|w| w.code == "OCR_TOTAL_UNVERIFIED"));
    assert_eq!(summarize(&q, Source::Ocr), QuoteTrust::Uncertain);
}

/// 품목 합이 검증되면 (주산암산처럼) 지금까지대로 '확인 필요' 다
#[test]
fn fallback_total_with_verifiable_items_stays_needs_check() {
    let mut q = table::interpret(&table_of(&[
        &["합계금액", "鬧8,000"],
        &["품명", "규격", "수량", "단가", "금액"],
        &["교재A", "권", "3", "10,000", "30,000"],
    ]));
    apply(&mut q, Source::Ocr);
    assert!(!q.warnings.iter().any(|w| w.code == "OCR_TOTAL_UNVERIFIED"));
    assert_eq!(summarize(&q, Source::Ocr), QuoteTrust::NeedsCheck);
}
