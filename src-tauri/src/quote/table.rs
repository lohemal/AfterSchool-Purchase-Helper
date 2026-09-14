//! 공통 표 해석 (설계안 6-1·6-2). **모든 형식이 이 한 벌을 지나간다.**
//!
//! 절차: 표 고르기 → 머리글 찾기 → 열 역할 → 행 분류 → 숫자 정규화 → 합계 추출 → 할인 부호 검산

use crate::domain::{CompareBasis, Confidence, RowKind, SignEffect, Warning};
use crate::quote::model::{ParsedItem, ParsedQuote, RawTable};
use crate::quote::number;

// ---------------------------------------------------------------- 낱말 사전

/// 열 역할. 설정으로 넓힐 수 있게 한곳에 모아 둔다.
const NAME_WORDS: &[&str] =
    &["품명", "품목", "물품명", "상품명", "내역", "품명/규격", "품목명", "내용"];
const SPEC_WORDS: &[&str] = &["규격", "사양", "단위", "규격/단위"];
const QTY_WORDS: &[&str] = &["수량", "수 량"];
const PRICE_WORDS: &[&str] = &["단가", "예상단가", "단 가"];
const AMOUNT_WORDS: &[&str] = &["금액", "공급가액", "공급가", "공급가 액"];
const TAX_WORDS: &[&str] = &["세액", "부가세", "세 액", "부가가치세"];
const NOTE_WORDS: &[&str] = &["비고", "적요", "비 고"];
const NO_WORDS: &[&str] = &["순번", "번호", "no", "연번"];

/// 합계·소계 행을 가리키는 낱말
const TOTAL_WORDS: &[&str] = &["합계", "총계", "소계", "계", "총액", "부가세", "vat", "합 계"];

/// 할인·금액 조정 행을 가리키는 낱말 (설계안 6-2)
const ADJUST_WORDS: &[&str] =
    &["할인", "할인금액", "dc", "d/c", "에누리", "차감", "조정", "감액", "디씨"];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Role {
    No,
    Name,
    Spec,
    Qty,
    UnitPrice,
    Amount,
    Tax,
    Note,
}

fn squash(s: &str) -> String {
    s.chars().filter(|c| !c.is_whitespace()).collect::<String>().to_lowercase()
}

/// 열 이름으로 쓰이는 모든 낱말 (머리글 조각을 도로 붙일 때 쓴다)
const ROLE_WORD_LISTS: &[&[&str]] =
    &[NAME_WORDS, SPEC_WORDS, QTY_WORDS, PRICE_WORDS, AMOUNT_WORDS, TAX_WORDS, NOTE_WORDS, NO_WORDS];

/// 글자를 더 붙이면 열 이름이 **될 수 있는가** (`품` → `품명`).
///
/// PDF 는 머리글을 칸 너비에 맞춰 벌려 쓰기 때문에 `품`·`명` 이 따로 떨어져 나온다.
/// 사전에 **정확히 맞는 만큼만** 붙이므로 없는 낱말을 지어내지 않는다.
pub fn is_role_prefix(s: &str) -> bool {
    let h = squash(s);
    if h.is_empty() {
        return false;
    }
    ROLE_WORD_LISTS
        .iter()
        .flat_map(|l| l.iter())
        .any(|w| {
            let w = squash(w);
            w.len() > h.len() && w.starts_with(&h)
        })
}

pub fn role_of(header: &str) -> Option<Role> {
    let h = squash(header);
    if h.is_empty() {
        return None;
    }
    let hit = |list: &[&str]| list.iter().any(|w| squash(w) == h);
    if hit(NAME_WORDS) {
        return Some(Role::Name);
    }
    if hit(SPEC_WORDS) {
        return Some(Role::Spec);
    }
    if hit(QTY_WORDS) {
        return Some(Role::Qty);
    }
    if hit(PRICE_WORDS) {
        return Some(Role::UnitPrice);
    }
    if hit(AMOUNT_WORDS) {
        return Some(Role::Amount);
    }
    if hit(TAX_WORDS) {
        return Some(Role::Tax);
    }
    if hit(NOTE_WORDS) {
        return Some(Role::Note);
    }
    if hit(NO_WORDS) {
        return Some(Role::No);
    }
    None
}

fn is_total_row(name: &str) -> bool {
    let n = squash(name);
    !n.is_empty() && TOTAL_WORDS.iter().any(|w| squash(w) == n)
}

fn is_adjust_row(name: &str) -> bool {
    let n = squash(name);
    !n.is_empty() && ADJUST_WORDS.iter().any(|w| n.contains(&squash(w)))
}

// ---------------------------------------------------------------- 표 고르기

/// 표가 여럿이면 **제목 글자로 고른다** (설계안 6-1 1번).
/// 견적서와 납품서는 행 수도 내용도 같을 수 있어 크기로는 못 고른다 (P0-5 실측).
pub fn pick_table(tables: &[RawTable]) -> (usize, String, Option<Warning>) {
    if tables.len() <= 1 {
        return (0, "표가 하나뿐입니다.".into(), None);
    }
    for (i, t) in tables.iter().enumerate() {
        if squash(&t.title).contains("견적서") {
            return (i, "제목이 '견적서' 인 표를 골랐습니다.".into(), None);
        }
    }
    for (i, t) in tables.iter().enumerate() {
        let n = squash(&t.title);
        if !n.contains("납품서") && !n.contains("거래명세") && !n.contains("거래명세서") {
            return (
                i,
                "납품서가 아닌 첫 표를 골랐습니다.".into(),
                Some(Warning::warn(
                    "MULTIPLE_TABLES",
                    "표가 여러 개입니다. 고른 표가 맞는지 확인해 주세요.",
                )),
            );
        }
    }
    (
        0,
        "표를 제목으로 고르지 못해 첫 표를 썼습니다.".into(),
        Some(Warning::warn("MULTIPLE_TABLES", "표가 여러 개입니다. 고른 표가 맞는지 확인해 주세요.")),
    )
}

// ---------------------------------------------------------------- 본 해석

pub fn interpret(table: &RawTable) -> ParsedQuote {
    let mut warnings: Vec<Warning> = Vec::new();
    let width = table.width();

    // 1) 머리글 행 — 낱말이 가장 많이 맞는 행. **행 번호를 고정하지 않는다.**
    let mut header_row = 0usize;
    let mut best_hits = 0usize;
    for r in 0..table.rows.len() {
        let hits = (0..width).filter(|c| role_of(table.get(r, *c)).is_some()).count();
        if hits > best_hits {
            best_hits = hits;
            header_row = r;
        }
    }
    if best_hits < 2 {
        warnings.push(Warning::warn(
            "HEADER_AMBIGUOUS",
            "머리글을 찾지 못했습니다. 품목을 직접 확인해 주세요.",
        ));
        return empty_quote(warnings);
    }

    // 2) 열 역할. 같은 역할이 여럿이면 먼저 나온 것을 쓴다.
    let mut roles: Vec<(usize, Role)> = Vec::new();
    for c in 0..width {
        if let Some(role) = role_of(table.get(header_row, c)) {
            if !roles.iter().any(|(_, r)| *r == role) {
                roles.push((c, role));
            }
        }
    }
    roles.sort_by_key(|(c, _)| *c);

    // 3) 열 범위. **병합된 열은 머리글 칸과 값 칸의 차례가 다르다** (P0-2 실측).
    //    역할 머리글 차례부터 다음 역할 머리글 차례 직전까지를 그 열의 범위로 잡는다.
    let bounds: Vec<(usize, usize, Role)> = roles
        .iter()
        .enumerate()
        .map(|(i, (c, role))| {
            let end = roles.get(i + 1).map(|(nc, _)| *nc).unwrap_or(width);
            (*c, end.max(*c + 1), *role)
        })
        .collect();

    let value_of = |r: usize, want: Role| -> (String, String) {
        for (start, end, role) in &bounds {
            if *role == want {
                for c in *start..*end {
                    if let Some(cell) = table.cell(r, c) {
                        if !cell.text.trim().is_empty() {
                            return (cell.text.trim().to_string(), cell.cell_ref.clone());
                        }
                    }
                }
                // 값이 없으면 첫 칸의 위치라도 돌려준다
                let cr = table.cell(r, *start).map(|c| c.cell_ref.clone()).unwrap_or_default();
                return (String::new(), cr);
            }
        }
        (String::new(), String::new())
    };

    let has_role = |want: Role| bounds.iter().any(|(_, _, r)| *r == want);
    if !has_role(Role::Name) {
        warnings.push(Warning::warn("HEADER_AMBIGUOUS", "품명 열을 찾지 못했습니다."));
    }
    if !has_role(Role::Amount) {
        warnings.push(Warning::warn("HEADER_AMBIGUOUS", "금액 열을 찾지 못했습니다."));
    }

    // 4) 행 분류 (설계안 6-2 — 다섯 종류)
    let mut items: Vec<ParsedItem> = Vec::new();
    let mut total_row_amounts: Vec<i64> = Vec::new();
    let mut total_row_raw: Vec<String> = Vec::new();
    let mut tax_row_amount: Option<i64> = None;

    for r in (header_row + 1)..table.rows.len() {
        let (name, name_ref) = value_of(r, Role::Name);
        let (spec, _) = value_of(r, Role::Spec);
        let (qty_s, _) = value_of(r, Role::Qty);
        let (price_s, _) = value_of(r, Role::UnitPrice);
        let (amount_s, _) = value_of(r, Role::Amount);

        // 합계/소계 행 — 총액 추출에 쓰고 품목으로 저장하지 않는다
        if is_total_row(&name) {
            let squashed = squash(&name);
            if let Some(a) = number::parse(&amount_s) {
                if squashed.contains("부가세") || squashed == "vat" {
                    tax_row_amount = Some(a);
                } else {
                    total_row_amounts.push(a);
                    total_row_raw.push(amount_s.clone());
                }
            }
            continue;
        }

        let amount = number::parse(&amount_s);
        let qty = number::parse(&qty_s);
        let unit_price = number::parse(&price_s);

        // 빈 행 — 이름도 금액도 없다. 저장하지 않는다.
        if name.trim().is_empty() && amount.is_none() {
            continue;
        }
        if name.trim().is_empty() {
            // 이름 없이 금액만 있는 줄 — 판단 불가. 경고하고 넘어간다.
            continue;
        }

        // 종류 정하기
        let kind = if is_adjust_row(&name) || amount.map(|a| a < 0).unwrap_or(false) {
            RowKind::Adjustment
        } else if amount.unwrap_or(0) == 0 {
            // 금액 0원 행(사은품 등) — 참고용으로 남기되 계산에서 뺀다 (14장 10번)
            RowKind::Zero
        } else {
            RowKind::Item
        };

        let mut row_warnings: Vec<Warning> = Vec::new();
        if kind == RowKind::Item {
            if let (Some(q), Some(u), Some(a)) = (qty, unit_price, amount) {
                let calc = q.saturating_mul(u);
                let diff = (calc - a).abs();
                if diff > 0 {
                    if diff < 10 {
                        row_warnings.push(Warning::info(
                            "QTY_PRICE_MISMATCH",
                            format!("수량×단가 {calc} 과 금액 {a} 이 {diff}원 다릅니다(절사로 보입니다)."),
                        ));
                    } else {
                        row_warnings.push(Warning::warn(
                            "QTY_PRICE_MISMATCH",
                            format!("수량×단가 {calc} 과 금액 {a} 이 맞지 않습니다."),
                        ));
                    }
                }
            }
        }
        if amount.is_none() && !amount_s.trim().is_empty() {
            row_warnings
                .push(Warning::warn("NUMBER_UNPARSED", "금액을 숫자로 읽지 못했습니다."));
        }

        items.push(ParsedItem {
            kind,
            sign_effect: if kind == RowKind::Adjustment {
                Some(SignEffect::Unknown) // 아래에서 검산으로 정한다
            } else {
                None
            },
            display_name: name.clone(),
            spec: spec.clone(),
            qty,
            unit_price,
            amount,
            raw_name: name,
            raw_spec: spec,
            raw_qty: qty_s,
            raw_unit_price: price_s,
            raw_amount: amount_s,
            cell_ref: name_ref,
            confidence: Confidence::High,
            warnings: row_warnings,
        });
    }

    // 5) 표기 합계 뽑기
    let stated = find_stated_totals(table, header_row, width);
    let mut supply_total = stated.supply;
    let grand_total = stated.grand;
    let tax_total = stated.tax.or(tax_row_amount);

    // 합계 행의 값은 공급가액으로 본다 (표기 합계가 따로 없을 때)
    let mut supply_raw = stated.supply_raw.clone();
    if supply_total.is_none() {
        if let Some(a) = total_row_amounts.first() {
            supply_total = Some(*a);
            supply_raw = total_row_raw.first().cloned().unwrap_or_default();
        }
    }

    // 6) 할인 부호 검산 (설계안 6-2)
    let item_sum: i64 = items
        .iter()
        .filter(|i| i.kind == RowKind::Item)
        .filter_map(|i| i.amount)
        .sum();

    let adjust_raw: i64 = items
        .iter()
        .filter(|i| i.kind == RowKind::Adjustment)
        .filter_map(|i| i.amount)
        .sum();
    let adjust_abs: i64 = items
        .iter()
        .filter(|i| i.kind == RowKind::Adjustment)
        .filter_map(|i| i.amount)
        .map(|a| a.abs())
        .sum();

    let has_adjust = items.iter().any(|i| i.kind == RowKind::Adjustment);
    let sign_effect = if !has_adjust {
        SignEffect::AsWritten
    } else {
        let target = grand_total.or(supply_total);
        match target {
            Some(t) if item_sum + adjust_raw == t => SignEffect::AsWritten,
            Some(t) if item_sum - adjust_abs == t => SignEffect::Subtract,
            _ => {
                warnings.push(Warning::warn(
                    "ADJUSTMENT_SIGN_UNKNOWN",
                    "할인 금액이 총액에 더해지는지 빼지는지 알 수 없습니다. 직접 골라 주세요.",
                ));
                SignEffect::Unknown
            }
        }
    };
    for it in items.iter_mut() {
        if it.kind == RowKind::Adjustment {
            it.sign_effect = Some(sign_effect);
        }
    }

    let adjustment_sum: i64 = items
        .iter()
        .filter(|i| i.kind == RowKind::Adjustment)
        .filter_map(|i| i.amount.map(|a| sign_effect.apply(a)))
        .sum();
    let computed_total = item_sum + adjustment_sum;

    // 7) 비교 기준
    let compare_basis = if grand_total.is_some() {
        CompareBasis::Grand
    } else if supply_total.is_some() {
        CompareBasis::Supply
    } else {
        warnings.push(Warning::warn(
            "TOTAL_NOT_FOUND",
            "견적서에 적힌 합계를 찾지 못해 품목 합을 씁니다.",
        ));
        CompareBasis::ComputedTotal
    };

    // 8) 품목 합과 표기 공급가액이 다르면 알린다
    if let Some(sup) = supply_total {
        if sup != computed_total {
            warnings.push(Warning::warn(
                "ITEM_SUM_NE_TOTAL",
                format!("품목 합 {computed_total} 과 견적서의 공급가액 {sup} 이 다릅니다."),
            ));
        }
    }
    if items.iter().all(|i| i.kind != RowKind::Item) {
        warnings.push(Warning::warn("EMPTY_ITEMS", "품목을 하나도 찾지 못했습니다."));
    }

    ParsedQuote {
        items,
        supply_total,
        tax_total,
        grand_total,
        raw_grand_total: stated.grand_raw.clone(),
        raw_supply_total: supply_raw,
        item_sum,
        adjustment_sum,
        computed_total,
        compare_basis,
        vendor_name_in_doc: find_vendor_name(table),
        table_note: String::new(),
        source: String::new(),
        trust: String::new(),
        warnings,
    }
}

fn empty_quote(warnings: Vec<Warning>) -> ParsedQuote {
    ParsedQuote {
        items: Vec::new(),
        supply_total: None,
        tax_total: None,
        grand_total: None,
        raw_grand_total: String::new(),
        raw_supply_total: String::new(),
        item_sum: 0,
        adjustment_sum: 0,
        computed_total: 0,
        compare_basis: CompareBasis::ComputedTotal,
        vendor_name_in_doc: String::new(),
        table_note: String::new(),
        source: String::new(),
        trust: String::new(),
        warnings,
    }
}

#[derive(Default)]
struct StatedTotals {
    supply: Option<i64>,
    tax: Option<i64>,
    grand: Option<i64>,
    /// `grand` 를 읽어 온 글자 그대로 (사진에서 깨졌는지 보는 근거)
    grand_raw: String,
    supply_raw: String,
}

/// 표 안팎에서 `합계금액` 같은 낱말 옆의 숫자를 찾는다.
fn find_stated_totals(table: &RawTable, header_row: usize, width: usize) -> StatedTotals {
    let mut out = StatedTotals::default();

    // 머리글 위쪽 + 표 밖 글자에서 찾는다 (품목 행과 섞이지 않게)
    let mut texts: Vec<(String, usize, usize)> = Vec::new();
    for r in 0..header_row.min(table.rows.len()) {
        for c in 0..width {
            let t = table.get(r, c);
            if !t.trim().is_empty() {
                texts.push((t.to_string(), r, c));
            }
        }
    }
    for cell in &table.loose_text {
        if !cell.text.trim().is_empty() {
            texts.push((cell.text.clone(), usize::MAX, usize::MAX));
        }
    }

    for (i, (t, r, c)) in texts.iter().enumerate() {
        let s = squash(t);
        let is_grand_label = s.contains("합계금액") || s.contains("합계금") || s.contains("총금액");
        if !is_grand_label {
            continue;
        }
        // 같은 글자 안에 숫자가 있으면 그것
        if let Some(n) = number::parse(t) {
            out.grand = Some(n);
            out.grand_raw = t.clone();
            break;
        }
        // 같은 행 오른쪽에서
        if *r != usize::MAX {
            let mut found = None;
            for cc in (*c + 1)..width {
                if let Some(n) = number::parse(table.get(*r, cc)) {
                    found = Some(n);
                    out.grand_raw = table.get(*r, cc).to_string();
                    break;
                }
            }
            if found.is_none() {
                // 아래 몇 줄에서
                for rr in (*r + 1)..(r + 3).min(header_row) {
                    for cc in 0..width {
                        if let Some(n) = number::parse(table.get(rr, cc)) {
                            found = Some(n);
                            out.grand_raw = table.get(rr, cc).to_string();
                            break;
                        }
                    }
                    if found.is_some() {
                        break;
                    }
                }
            }
            if let Some(n) = found {
                out.grand = Some(n);
                break;
            }
        } else {
            // 표 밖 글자 — 뒤따르는 조각에서 찾는다
            for (t2, _, _) in texts.iter().skip(i + 1).take(4) {
                if let Some(n) = number::parse(t2) {
                    out.grand = Some(n);
                    out.grand_raw = t2.clone();
                    break;
                }
            }
            if out.grand.is_some() {
                break;
            }
        }
    }
    out
}

/// 견적서 양식의 칸 이름들. 값이 비었을 때 **옆 칸의 이름표를 상호로 잘못 읽지 않도록** 쓴다.
const FORM_LABELS: &[&str] = &[
    "상호", "상호(법인명)", "회사명", "공급자", "성명", "대표자", "등록번호", "사업자등록번호",
    "사업장등록번호", "사업장주소", "사업장소재지", "소재지", "주소", "업태", "종목", "전화번호",
    "연락처", "팩스", "일자", "귀하", "No", "번호",
];

fn is_form_label(s: &str) -> bool {
    let n = squash(s);
    !n.is_empty() && FORM_LABELS.iter().any(|w| squash(w) == n)
}

/// 도장·기호 같은 사용자 정의 영역 글자만 있는 칸인가 (한글 문서의 (인) 도장 등)
fn is_symbol_only(s: &str) -> bool {
    !s.is_empty()
        && s.chars()
            .all(|c| c.is_whitespace() || ('\u{E000}'..='\u{F8FF}').contains(&c) || ('\u{F0000}'..='\u{FFFFD}').contains(&c))
}

/// 견적서 안의 상호 (참고용. 매칭에는 쓰지 않는다 — 설계안 6-4).
/// **칸이 비어 있으면 빈 값을 돌려준다.** 옆에 있는 다른 이름표를 끌어오지 않는다.
fn find_vendor_name(table: &RawTable) -> String {
    let width = table.width();
    for r in 0..table.rows.len().min(20) {
        for c in 0..width {
            let s = squash(table.get(r, c));
            if s == "상호" || s == "상호(법인명)" || s == "회사명" || s == "공급자" {
                for cc in (c + 1)..width {
                    let v = table.get(r, cc).trim();
                    if v.is_empty() {
                        continue;
                    }
                    // 다음 이름표를 만나면 이 칸의 값은 비어 있는 것이다 — 멈춘다
                    if is_form_label(v) {
                        break;
                    }
                    if is_symbol_only(v) {
                        continue;
                    }
                    return v.to_string();
                }
            }
        }
    }
    String::new()
}

#[cfg(test)]
#[path = "table_tests.rs"]
mod table_tests;
