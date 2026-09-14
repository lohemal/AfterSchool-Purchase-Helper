//! P1-6 정산자료 읽기 (P0-3 에서 검증한 방식, 설계안 8-2·8-3).
//!
//! - 머리글 행 번호를 고정하지 않고 낱말로 찾는다.
//! - **필요한 열을 못 찾으면 추정하지 말고 멈춘다.**
//! - A열이 `합계` 인 행은 자료에서 빼되, 열별 합이 그 행과 맞는지 **검산해 형식 확인에 쓴다.**
//! - 정산 별칭 → 품의 부서 연결은 `settlement_alias` 표에서만 한다. 문자열 규칙으로 추측하지 않는다.

use std::path::Path;

use calamine::{open_workbook_auto, Data, Reader};
use serde::{Deserialize, Serialize};

use crate::domain::{Funds, Warning};
use crate::error::{AppError, AppResult};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SettlementRow {
    pub row_no: i64,
    /// 정산자료 '부서명' 열의 **원문**
    pub source_name: String,
    pub funds: Funds,
    /// 정산자료에 적힌 합계 (있으면)
    pub stated_total: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SettlementFile {
    pub sheet_name: String,
    pub header_row: i64,
    pub rows: Vec<SettlementRow>,
    /// 합계 행 (있으면)
    pub stated_totals: Option<Funds>,
    pub warnings: Vec<Warning>,
}

fn squash(s: &str) -> String {
    s.chars().filter(|c| !c.is_whitespace()).collect::<String>().to_lowercase()
}

fn text(d: Option<&Data>) -> String {
    match d {
        Some(Data::String(s)) => s.trim().to_string(),
        Some(Data::Float(f)) => format!("{}", *f as i64),
        Some(Data::Int(i)) => i.to_string(),
        _ => String::new(),
    }
}

fn number(d: Option<&Data>) -> Option<i64> {
    match d {
        Some(Data::Float(f)) => Some(f.round() as i64),
        Some(Data::Int(i)) => Some(*i),
        Some(Data::String(s)) => crate::quote::number::parse(s),
        _ => None,
    }
}

fn is_name_header(h: &str) -> bool {
    matches!(squash(h).as_str(), "부서명" | "부서" | "강좌명" | "구분" | "프로그램명")
}

pub fn is_total_label(h: &str) -> bool {
    matches!(squash(h).as_str(), "합계" | "계" | "총계")
}

/// 재원 열 판별. 열 이름이 학교마다 조금씩 달라 **포함**으로 본다
/// (예: `수익자(1,2,4,5,6학년)` · `3학년 초과금`).
fn fund_of(h: &str) -> Option<usize> {
    let s = squash(h);
    if s.contains("수익자") {
        Some(0)
    } else if s.contains("초과금") {
        Some(1)
    } else if s.contains("지원금") {
        Some(2)
    } else if s.contains("자유수강권") {
        Some(3)
    } else {
        None
    }
}

pub fn read(path: &Path) -> AppResult<SettlementFile> {
    let mut wb = open_workbook_auto(path).map_err(|e| {
        AppError::new("SETTLE_OPEN_FAILED", "정산자료 파일을 열지 못했습니다.").detail(e.to_string())
    })?;

    let sheets = wb.sheet_names().to_vec();
    let mut last_err: Option<AppError> = None;

    for name in sheets {
        let Ok(range) = wb.worksheet_range(&name) else { continue };
        if range.is_empty() {
            continue;
        }
        match parse_range(&range, &name) {
            Ok(f) => return Ok(f),
            Err(e) => last_err = Some(e),
        }
    }
    Err(last_err.unwrap_or_else(|| {
        AppError::new("SETTLE_NO_SHEET", "정산자료에서 읽을 수 있는 시트를 찾지 못했습니다.")
    }))
}

fn parse_range(range: &calamine::Range<Data>, sheet: &str) -> AppResult<SettlementFile> {
    let (row0, _) = range.start().unwrap_or((0, 0));
    let (h, w) = (range.height(), range.width());

    // 머리글 — 위에서 10행 안에서 낱말로 찾는다 (행 번호를 고정하지 않는다)
    let mut header = None;
    for r in 0..h.min(10) {
        let has_name = (0..w).any(|c| is_name_header(&text(range.get((r, c)))));
        let funds = (0..w).filter(|c| fund_of(&text(range.get((r, *c)))).is_some()).count();
        if has_name && funds >= 3 {
            header = Some(r);
            break;
        }
    }
    let hr = header.ok_or_else(|| {
        AppError::new(
            "SETTLE_HEADER_NOT_FOUND",
            "정산자료 형식이 다릅니다: 부서명과 재원 열이 있는 머리글 줄을 찾지 못했습니다.",
        )
    })?;

    let name_col = (0..w)
        .find(|c| is_name_header(&text(range.get((hr, *c)))))
        .ok_or_else(|| AppError::new("SETTLE_NO_NAME_COL", "정산자료에 부서명 열이 없습니다."))?;

    // 재원 네 열을 모두 찾아야 한다. 하나라도 없으면 **멈춘다** (설계안 8-3)
    let mut fund_cols = [usize::MAX; 4];
    for c in 0..w {
        if let Some(i) = fund_of(&text(range.get((hr, c)))) {
            if fund_cols[i] == usize::MAX {
                fund_cols[i] = c;
            }
        }
    }
    let labels = ["수익자", "초과금", "지원금", "자유수강권"];
    let missing: Vec<&str> = (0..4).filter(|i| fund_cols[*i] == usize::MAX).map(|i| labels[i]).collect();
    if !missing.is_empty() {
        return Err(AppError::new(
            "SETTLE_MISSING_FUND_COL",
            format!("정산자료 형식이 다릅니다: 못 찾은 열 {}", missing.join(", ")),
        ));
    }

    let total_col = (0..w).find(|c| is_total_label(&text(range.get((hr, *c)))));

    // 자료 행과 합계 행 가르기
    let mut rows = Vec::new();
    let mut stated_totals: Option<Funds> = None;
    let mut stated_total_sum: Option<i64> = None;

    for r in (hr + 1)..h {
        let name = text(range.get((r, name_col)));
        if name.trim().is_empty() {
            continue;
        }
        let funds = Funds {
            beneficiary: number(range.get((r, fund_cols[0]))).unwrap_or(0),
            excess: number(range.get((r, fund_cols[1]))).unwrap_or(0),
            subsidy: number(range.get((r, fund_cols[2]))).unwrap_or(0),
            voucher: number(range.get((r, fund_cols[3]))).unwrap_or(0),
        };
        let stated = total_col.and_then(|c| number(range.get((r, c))));

        if is_total_label(&name) {
            stated_totals = Some(funds);
            stated_total_sum = stated;
            continue;
        }
        rows.push(SettlementRow {
            row_no: (row0 as usize + r + 1) as i64,
            source_name: name,
            funds,
            stated_total: stated,
        });
    }

    if rows.is_empty() {
        return Err(AppError::new("SETTLE_EMPTY", "정산자료에서 자료 줄을 찾지 못했습니다."));
    }

    // --- 검산 (형식이 맞는지 확인하는 용도. 값을 고치지 않는다) ---
    let mut warnings = Vec::new();
    for row in &rows {
        if let Some(t) = row.stated_total {
            if row.funds.total() != t {
                warnings.push(Warning::warn(
                    "SETTLE_ROW_SUM",
                    format!(
                        "{} — 재원 네 칸의 합 {} 과 적힌 합계 {} 이 다릅니다.",
                        row.source_name,
                        crate::domain::comma(row.funds.total()),
                        crate::domain::comma(t)
                    ),
                ));
            }
        }
    }
    if let Some(tot) = stated_totals {
        let mut sum = Funds::default();
        for row in &rows {
            sum.add(&row.funds);
        }
        for (got, want, label) in [
            (sum.beneficiary, tot.beneficiary, "수익자"),
            (sum.excess, tot.excess, "초과금"),
            (sum.subsidy, tot.subsidy, "지원금"),
            (sum.voucher, tot.voucher, "자유수강권"),
        ] {
            if got != want {
                warnings.push(Warning::warn(
                    "SETTLE_COLUMN_SUM",
                    format!(
                        "{label} 열의 합 {} 과 합계 줄의 {} 이 다릅니다.",
                        crate::domain::comma(got),
                        crate::domain::comma(want)
                    ),
                ));
            }
        }
        if let Some(t) = stated_total_sum {
            if sum.total() != t {
                warnings.push(Warning::warn(
                    "SETTLE_GRAND_SUM",
                    "모든 재원의 합과 합계 줄의 값이 다릅니다.",
                ));
            }
        }
    } else {
        warnings.push(Warning::info("SETTLE_NO_TOTAL_ROW", "정산자료에 합계 줄이 없습니다."));
    }

    Ok(SettlementFile {
        sheet_name: sheet.to_string(),
        header_row: (row0 as usize + hr + 1) as i64,
        rows,
        stated_totals,
        warnings,
    })
}

#[cfg(test)]
#[path = "settlement_tests.rs"]
mod settlement_tests;
