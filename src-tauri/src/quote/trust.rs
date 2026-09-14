//! P3 OCR 결과를 얼마나 믿을지 정한다.
//!
//! **Windows OCR 은 낱말 신뢰도를 주지 않는다.** 그래서 가짜 확률값을 만들어 내지 않고,
//! **실제로 검증할 수 있는 근거**만 쓴다 (설계안 7장).
//!
//!   - 필요한 열(품명·수량·단가·금액)을 찾았는가
//!   - 행마다 `수량 × 단가 == 금액` 이 성립하는가
//!   - 품목 합이 견적서에 적힌 합계와 맞는가
//!   - 글자에 이상한 기호가 섞여 있는가
//!
//! 어긋나는 값을 **고치지 않는다.** 예를 들어 `3 × 10,000 = 80,000` 이면
//! 30,000 이라고 추정하지 않고 "금액 검증 실패" 로 표시한다.
//!
//! 그리고 사진에서 읽은 것에는 **절대 `high` 를 주지 않는다.** 사람이 한 번은 봐야 한다.

use crate::domain::{comma, CompareBasis, Confidence, RowKind, Severity, Warning};
use crate::quote::model::ParsedQuote;
use crate::quote::table;

/// 읽어 온 경로 — 사진인지 아닌지에 따라 믿는 정도가 다르다
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Source {
    /// XLSX · HWP · HWPX — 값이 그대로 들어 있다
    Structured,
    /// 텍스트 PDF — 좌표로 표를 되살렸다
    PdfText,
    /// 사진·스캔 — OCR 로 읽었다
    Ocr,
}

/// 이상해 보이는 글자 (OCR 이 흔히 만드는 것)
fn looks_garbled(s: &str) -> bool {
    if s.trim().is_empty() {
        return false;
    }
    let odd = s
        .chars()
        .filter(|c| {
            // 한글 자모 낱자(완성되지 않은 글자) · 사용자 정의 영역 · 한자·기호 뭉치
            ('\u{3130}'..='\u{318F}').contains(c)
                || ('\u{E000}'..='\u{F8FF}').contains(c)
                || ('\u{FFF0}'..='\u{FFFF}').contains(c)
        })
        .count();
    odd > 0
}

/// 숫자 바로 앞뒤에 다른 글자가 **붙어 있으면** 온전히 읽힌 숫자가 아니다.
///
/// 실제 주산암산 사진에서 `₩68,000` 이 `鬧8,000` 으로 읽혔다.
/// 이것을 68,000 으로 되돌리는 것은 추정이므로 하지 않는다.
/// 대신 **그 합계를 읽지 못한 것으로 다룬다** (값은 지우지 않고 그대로 남긴다).
pub fn number_looks_broken(raw: &str) -> bool {
    // 숫자 앞에 와도 되는 것: 통화 기호 · 여는 괄호 · 부호 · 구분 기호
    const OK_BEFORE: &str = r#"₩￦Ww\:=(-+., "#;
    // 숫자 뒤에 와도 되는 것
    const OK_AFTER: &str = r#"원)-+., "#;

    let chars: Vec<char> = raw.trim().chars().collect();
    let Some(first) = chars.iter().position(|c| c.is_ascii_digit()) else {
        return true;
    };
    let last = chars.iter().rposition(|c| c.is_ascii_digit()).unwrap();

    let ok = |c: char, set: &str| c.is_whitespace() || set.contains(c);
    if first > 0 && !ok(chars[first - 1], OK_BEFORE) {
        return true;
    }
    if last + 1 < chars.len() && !ok(chars[last + 1], OK_AFTER) {
        return true;
    }
    // 숫자 구간 안에 다른 글자가 섞여도 깨진 것
    chars[first..=last]
        .iter()
        .any(|c| !(c.is_ascii_digit() || *c == ',' || *c == '.' || c.is_whitespace()))
}

/// 사진에서 읽은 결과에 근거 기반 신뢰도와 경고를 붙인다.
///
/// **값은 하나도 바꾸지 않는다.** 표시만 더한다.
pub fn apply(quote: &mut ParsedQuote, source: Source) {
    if source == Source::Structured {
        return;
    }

    let is_ocr = source == Source::Ocr;
    let label = if is_ocr { "사진에서 읽은 값입니다" } else { "PDF 좌표로 되살린 값입니다" };

    // --- 행마다 ---
    let mut any_mismatch = false;
    // 모든 품목 줄에서 수량×단가=금액 이 확인됐는가 (품목 합을 믿을 근거)
    let mut every_item_verified = true;
    let mut item_rows = 0usize;
    for item in quote.items.iter_mut() {
        let mut reasons: Vec<&str> = Vec::new();

        if looks_garbled(&item.display_name) || looks_garbled(&item.spec) {
            reasons.push("글자가 깨져 보입니다");
            item.warnings.push(Warning::warn(
                "OCR_GARBLED",
                "글자가 제대로 읽히지 않았을 수 있습니다. 물품명을 확인해 주세요.",
            ));
        }

        if item.kind == RowKind::Item {
            item_rows += 1;
            match (item.qty, item.unit_price, item.amount) {
                (Some(q), Some(u), Some(a)) if q.saturating_mul(u) == a => {
                    // 수량×단가=금액 이 맞으면 숫자를 제대로 읽었다는 강한 근거다
                }
                (Some(_), Some(_), Some(_)) => {
                    any_mismatch = true;
                    every_item_verified = false;
                    reasons.push("수량×단가와 금액이 맞지 않습니다");
                }
                _ => {
                    every_item_verified = false;
                    reasons.push("수량·단가·금액 중 읽지 못한 값이 있습니다");
                }
            }
        }

        item.confidence = if reasons.is_empty() {
            // 검증을 통과해도 사진은 medium 까지다 — 사람이 봐야 한다
            if is_ocr {
                Confidence::Medium
            } else {
                Confidence::High
            }
        } else {
            Confidence::Low
        };

        if is_ocr && item.confidence == Confidence::Medium {
            item.warnings.push(Warning::info("OCR_CHECK", format!("{label}. 값을 확인해 주세요.")));
        }
    }

    // --- 견적서 전체 ---
    let missing_roles = missing_required_roles(quote);
    if !missing_roles.is_empty() {
        quote.warnings.push(Warning::warn(
            "OCR_MISSING_COLUMN",
            format!("{} 열을 알아보지 못했습니다. 값을 확인해 주세요.", missing_roles.join("·")),
        ));
    }

    if any_mismatch {
        quote.warnings.push(Warning::warn(
            "OCR_AMOUNT_CHECK",
            "수량×단가와 금액이 맞지 않는 줄이 있습니다. 프로그램이 고치지 않았습니다.",
        ));
    }

    if is_ocr {
        quote.warnings.push(Warning::info(
            "OCR_USED",
            "사진에서 자동으로 읽었습니다. 품목과 금액을 한 번 확인해 주세요.",
        ));
    }

    // 사진에서 표기 합계가 **깨져** 읽혔는가.
    // P0-6 실측: `₩` 가 붙은 합계는 3배 확대에서도 깨진다. 되돌리지 않고 **쓰지 않는다**.
    // 읽은 값은 지우지 않으므로 화면에서 원문을 그대로 볼 수 있다.
    let mut total_fell_back = false;
    if is_ocr {
        let broken_grand =
            quote.grand_total.is_some() && number_looks_broken(&quote.raw_grand_total);
        let broken_supply =
            quote.supply_total.is_some() && number_looks_broken(&quote.raw_supply_total);

        if (broken_grand && quote.compare_basis == CompareBasis::Grand)
            || (broken_supply && quote.compare_basis == CompareBasis::Supply)
        {
            let raw = if broken_grand { &quote.raw_grand_total } else { &quote.raw_supply_total };
            quote.warnings.push(Warning::warn(
                "OCR_TOTAL_UNREADABLE",
                format!(
                    "견적서에 적힌 합계({})를 제대로 읽지 못해 품목 합 {} 원을 씁니다. 총액을 꼭 확인해 주세요.",
                    raw.trim(),
                    comma(quote.computed_total)
                ),
            ));
            // 값은 그대로 두고, **무엇과 비교할지만** 품목 합으로 바꾼다
            quote.compare_basis = CompareBasis::ComputedTotal;
            total_fell_back = true;
        } else if let Some(stated) = match quote.compare_basis {
            CompareBasis::Grand => quote.grand_total,
            CompareBasis::Supply => quote.supply_total,
            CompareBasis::ComputedTotal => None,
        } {
            // 깨끗하게 읽힌 합계가 품목 합과 다르다 → 고치지 않고 검증 실패로 알린다
            if stated != quote.computed_total {
                quote.warnings.push(Warning::warn(
                    "OCR_TOTAL_MISMATCH",
                    format!(
                        "견적서에 적힌 합계 {} 원과 품목 합 {} 원이 다릅니다. 프로그램이 고치지 않았습니다.",
                        comma(stated),
                        comma(quote.computed_total)
                    ),
                ));
            }
        }
    }

    // 표기 합계를 못 읽는 것은 사진에서 흔하다 (P0-6: `₩68,000` → 깨짐). 정상 경로다.
    if quote.grand_total.is_none() && quote.supply_total.is_none() && is_ocr {
        total_fell_back = true;
        quote.warnings.push(Warning::info(
            "OCR_TOTAL_FALLBACK",
            "견적서에 적힌 합계를 읽지 못해 품목 합을 씁니다. 총액을 확인해 주세요.",
        ));
    }

    // **품목 합조차 검증되지 않으면 정상으로 두지 않는다** (2026-09-13 승인 규칙).
    // 표기 합계를 못 써서 품목 합으로 비교하는데 그 품목 합도 맞춰 볼 근거가 없으면,
    // 대조할 것이 하나도 없다는 뜻이다.
    if is_ocr && total_fell_back && (item_rows == 0 || !every_item_verified) {
        quote.warnings.push(Warning::warn(
            "OCR_TOTAL_UNVERIFIED",
            "견적서에 적힌 합계를 쓰지 못했고 품목 합도 수량×단가로 맞춰 볼 수 없습니다. \
             금액을 원본과 직접 대조해 주세요.",
        ));
    }
}

/// 있어야 하는 열 중 못 찾은 것
fn missing_required_roles(quote: &ParsedQuote) -> Vec<&'static str> {
    let mut out = Vec::new();
    if quote.items.iter().all(|i| i.display_name.trim().is_empty()) {
        out.push("품명");
    }
    if quote.items.iter().filter(|i| i.kind == RowKind::Item).all(|i| i.amount.is_none()) {
        out.push("금액");
    }
    out
}

/// 화면에 보여 줄 한 줄 요약 상태
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QuoteTrust {
    /// 경고 없음
    Ok,
    /// 사람이 봐야 함 (사진에서 읽음)
    NeedsCheck,
    /// 글자가 깨졌거나 열을 못 찾음
    Uncertain,
    /// 수량×단가와 금액이 맞지 않음
    AmountFailed,
}

impl QuoteTrust {
    pub fn label(self) -> &'static str {
        match self {
            QuoteTrust::Ok => "정상",
            QuoteTrust::NeedsCheck => "확인 필요",
            QuoteTrust::Uncertain => "인식 불확실",
            QuoteTrust::AmountFailed => "금액 검증 실패",
        }
    }
    pub fn key(self) -> &'static str {
        match self {
            QuoteTrust::Ok => "ok",
            QuoteTrust::NeedsCheck => "needs_check",
            QuoteTrust::Uncertain => "uncertain",
            QuoteTrust::AmountFailed => "amount_failed",
        }
    }
}

/// 경고와 행 상태를 모아 한 줄 상태를 만든다. **나쁜 쪽이 이긴다.**
pub fn summarize(quote: &ParsedQuote, source: Source) -> QuoteTrust {
    let has = |code: &str| quote.warnings.iter().any(|w| w.code == code);

    if has("OCR_AMOUNT_CHECK")
        || has("OCR_TOTAL_MISMATCH")
        || quote
            .items
            .iter()
            .any(|i| i.warnings.iter().any(|w| w.code == "QTY_PRICE_MISMATCH" && w.severity == Severity::Warn))
    {
        return QuoteTrust::AmountFailed;
    }
    if has("OCR_MISSING_COLUMN")
        || has("OCR_TOTAL_UNVERIFIED")
        || has("HEADER_AMBIGUOUS")
        || quote.items.iter().any(|i| i.warnings.iter().any(|w| w.code == "OCR_GARBLED"))
    {
        return QuoteTrust::Uncertain;
    }
    if source == Source::Ocr {
        return QuoteTrust::NeedsCheck;
    }
    if quote.warnings.iter().any(|w| w.severity == Severity::Warn) {
        return QuoteTrust::NeedsCheck;
    }
    QuoteTrust::Ok
}

/// `table::interpret` 이 쓰는 열 이름 사전을 그대로 써서 머리글을 알아봤는지 본다
pub fn header_hits(cells: &[String]) -> usize {
    cells.iter().filter(|c| table::role_of(c).is_some()).count()
}

#[cfg(test)]
#[path = "trust_tests.rs"]
mod trust_tests;
