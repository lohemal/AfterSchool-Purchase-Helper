//! P1-7 거래처별 재원 배분과 이중 검증 (설계안 9장, 14장 5·6번).
//!
//! **이 파일은 금액을 추정하지 않는다.** 자동 비율 배분은 구현 자체가 없다.
//!   - 부서에 거래처가 하나면 부서 금액을 **그대로 옮긴다**(계산이 아니라 이동이다).
//!   - 둘 이상이면 사용자가 재원 네 칸을 각각 넣는다. 프로그램은 검사만 한다.
//!
//! 검증은 두 방향이다.
//!   세로 — 거래처별 같은 재원의 합 == 정산자료의 그 부서 재원 금액 (4개 각각)
//!   가로 — 한 거래처의 재원 4개 합 == 그 거래처 견적서 총액

use serde::{Deserialize, Serialize};

use crate::domain::{comma, Fund, Funds, ALL_FUNDS};

/// 한 부서의 배분 상황
#[derive(Debug, Clone)]
pub struct DeptAllocation {
    pub department_id: i64,
    pub department_name: String,
    /// 정산자료에서 이 부서로 모인 금액 (별칭 여럿이면 이미 합산된 값)
    pub settlement: Funds,
    /// 정산 행이 하나라도 있었는가
    pub has_settlement: bool,
    pub vendors: Vec<VendorAllocation>,
}

#[derive(Debug, Clone)]
pub struct VendorAllocation {
    pub vendor_unit_id: i64,
    pub mgmt_name: String,
    /// 사용자가 넣었거나 자동으로 옮긴 금액
    pub allocated: Funds,
    /// 배분 값이 아직 저장된 적 없는가
    pub missing: bool,
    /// 그 거래처 견적서의 비교 기준 총액. 견적서가 없으면 None.
    pub quote_total: Option<i64>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum CheckStatus {
    Ok,
    Warn,
    Error,
}

impl CheckStatus {
    pub fn key(self) -> &'static str {
        match self {
            CheckStatus::Ok => "ok",
            CheckStatus::Warn => "warn",
            CheckStatus::Error => "error",
        }
    }
}

/// 검증 한 건
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Check {
    /// vertical | horizontal | allocation_missing | missing_quote | missing_settlement
    pub kind: String,
    pub department_id: Option<i64>,
    pub vendor_unit_id: Option<i64>,
    pub fund: Option<String>,
    pub label: String,
    pub expected: Option<i64>,
    pub actual: Option<i64>,
    pub diff: Option<i64>,
    pub status: CheckStatus,
}

/// 거래처가 하나뿐인 부서의 자동 배분값.
/// **계산이 아니라 그대로 옮기는 것**이다. 거래처가 둘 이상이면 `None` 을 돌려준다.
pub fn auto_allocation(dept: &DeptAllocation) -> Option<(i64, Funds)> {
    let active: Vec<&VendorAllocation> = dept.vendors.iter().collect();
    if active.len() == 1 && dept.has_settlement {
        Some((active[0].vendor_unit_id, dept.settlement))
    } else {
        None
    }
}

/// 한 부서를 검증한다.
pub fn verify_department(dept: &DeptAllocation) -> Vec<Check> {
    let mut out = Vec::new();

    // 정산은 있는데 견적서가 하나도 없다
    if dept.has_settlement && dept.vendors.iter().all(|v| v.quote_total.is_none()) {
        out.push(Check {
            kind: "missing_quote".into(),
            department_id: Some(dept.department_id),
            vendor_unit_id: None,
            fund: None,
            label: format!("{} — 정산자료에는 있는데 견적서가 없습니다.", dept.department_name),
            expected: Some(dept.settlement.total()),
            actual: None,
            diff: None,
            status: CheckStatus::Error,
        });
    }

    // 견적서는 있는데 정산 행이 없다
    if !dept.has_settlement && dept.vendors.iter().any(|v| v.quote_total.is_some()) {
        out.push(Check {
            kind: "missing_settlement".into(),
            department_id: Some(dept.department_id),
            vendor_unit_id: None,
            fund: None,
            label: format!("{} — 견적서는 있는데 정산자료에 금액이 없습니다.", dept.department_name),
            expected: None,
            actual: None,
            diff: None,
            status: CheckStatus::Error,
        });
    }

    // 배분 미입력 (거래처가 둘 이상인데 값이 없다)
    for v in &dept.vendors {
        if v.missing && v.quote_total.is_some() {
            out.push(Check {
                kind: "allocation_missing".into(),
                department_id: Some(dept.department_id),
                vendor_unit_id: Some(v.vendor_unit_id),
                fund: None,
                label: format!("{} — 재원 배분을 아직 넣지 않았습니다.", v.mgmt_name),
                expected: v.quote_total,
                actual: None,
                diff: None,
                status: CheckStatus::Error,
            });
        }
    }

    // --- 세로: 재원마다 거래처 합 == 정산 금액 ---
    if dept.has_settlement && !dept.vendors.is_empty() {
        for f in ALL_FUNDS {
            let sum: i64 = dept.vendors.iter().map(|v| v.allocated.get(f)).sum();
            let want = dept.settlement.get(f);
            let diff = sum - want;
            out.push(Check {
                kind: "vertical".into(),
                department_id: Some(dept.department_id),
                vendor_unit_id: None,
                fund: Some(f.key().to_string()),
                label: format!(
                    "{} {} — 거래처 합 {} / 정산 {}",
                    dept.department_name,
                    f.label(),
                    comma(sum),
                    comma(want)
                ),
                expected: Some(want),
                actual: Some(sum),
                diff: Some(diff),
                status: if diff == 0 { CheckStatus::Ok } else { CheckStatus::Error },
            });
        }
    }

    // --- 가로: 거래처마다 재원 4개 합 == 견적 총액 ---
    for v in &dept.vendors {
        let Some(quote_total) = v.quote_total else { continue };
        let sum = v.allocated.total();
        let diff = sum - quote_total;
        out.push(Check {
            kind: "horizontal".into(),
            department_id: Some(dept.department_id),
            vendor_unit_id: Some(v.vendor_unit_id),
            fund: None,
            label: format!(
                "{} — 재원 합 {} / 견적 총액 {}",
                v.mgmt_name,
                comma(sum),
                comma(quote_total)
            ),
            expected: Some(quote_total),
            actual: Some(sum),
            diff: Some(diff),
            status: if diff == 0 { CheckStatus::Ok } else { CheckStatus::Error },
        });
    }

    out
}

/// 최종 품의 행 하나 (미리보기·생성이 함께 쓴다)
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PumuiRow {
    pub vendor_unit_id: i64,
    /// 내용 — 문구
    pub content: String,
    /// 예상단가 — 그 재원 금액
    pub amount: i64,
}

/// 재원 하나의 품의 행들. **금액이 0인 행은 만들지 않는다** (설계안 14장 14번).
pub fn rows_for_fund(
    fund: Fund,
    entries: &[(i64, String, Funds)], // (vendor_unit_id, 내용 문구, 배분액)
) -> Vec<PumuiRow> {
    entries
        .iter()
        .filter(|(_, _, f)| f.get(fund) != 0)
        .map(|(id, content, f)| PumuiRow {
            vendor_unit_id: *id,
            content: content.clone(),
            amount: f.get(fund),
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn funds(b: i64, e: i64, s: i64, v: i64) -> Funds {
        Funds { beneficiary: b, excess: e, subsidy: s, voucher: v }
    }

    fn vendor(id: i64, name: &str, alloc: Funds, quote: Option<i64>) -> VendorAllocation {
        VendorAllocation {
            vendor_unit_id: id,
            mgmt_name: name.into(),
            allocated: alloc,
            missing: false,
            quote_total: quote,
        }
    }

    /// 바둑 — 거래처 하나. 정산 금액을 그대로 옮기고 가로·세로가 다 맞는다.
    #[test]
    fn single_vendor_is_moved_not_computed() {
        let settlement = funds(372_000, 12_000, 60_000, 0);
        let dept = DeptAllocation {
            department_id: 1,
            department_name: "바둑".into(),
            settlement,
            has_settlement: true,
            vendors: vec![vendor(10, "바둑", Funds::default(), Some(444_000))],
        };
        let (vid, auto) = auto_allocation(&dept).expect("거래처 하나면 자동");
        assert_eq!(vid, 10);
        assert_eq!(auto, settlement, "계산하지 않고 그대로 옮긴다");

        let applied = DeptAllocation {
            vendors: vec![vendor(10, "바둑", auto, Some(444_000))],
            ..dept
        };
        let checks = verify_department(&applied);
        assert!(checks.iter().all(|c| c.status == CheckStatus::Ok), "{checks:#?}");
        assert_eq!(checks.iter().filter(|c| c.kind == "vertical").count(), 4);
        assert_eq!(checks.iter().filter(|c| c.kind == "horizontal").count(), 1);
    }

    /// 로봇과학 — 거래처 둘. **자동 배분을 내놓지 않는다.**
    #[test]
    fn two_vendors_never_auto_split() {
        let dept = DeptAllocation {
            department_id: 2,
            department_name: "로봇과학".into(),
            settlement: funds(810_000, 6_200, 443_800, 0),
            has_settlement: true,
            vendors: vec![
                vendor(20, "로봇과학1", Funds::default(), Some(900_000)),
                vendor(21, "로봇과학2", Funds::default(), Some(360_000)),
            ],
        };
        assert!(auto_allocation(&dept).is_none(), "거래처가 둘이면 자동 배분은 없다");
    }

    /// 사용자가 넣은 값이 세로·가로를 모두 만족하면 정상
    #[test]
    fn user_split_passes_both_directions() {
        let dept = DeptAllocation {
            department_id: 2,
            department_name: "로봇과학".into(),
            settlement: funds(810_000, 6_200, 443_800, 0),
            has_settlement: true,
            vendors: vec![
                vendor(20, "로봇과학1", funds(720_000, 6_200, 173_800, 0), Some(900_000)),
                vendor(21, "로봇과학2", funds(90_000, 0, 270_000, 0), Some(360_000)),
            ],
        };
        let checks = verify_department(&dept);
        let bad: Vec<&Check> = checks.iter().filter(|c| c.status != CheckStatus::Ok).collect();
        assert!(bad.is_empty(), "{bad:#?}");
    }

    /// 세로가 어긋나면 재원별로 정확히 짚어 준다
    #[test]
    fn vertical_mismatch_is_reported_per_fund() {
        let dept = DeptAllocation {
            department_id: 2,
            department_name: "로봇과학".into(),
            settlement: funds(810_000, 6_200, 443_800, 0),
            has_settlement: true,
            vendors: vec![
                vendor(20, "로봇과학1", funds(720_000, 6_200, 173_800, 0), Some(900_000)),
                // 수익자를 1,000 적게 넣었다
                vendor(21, "로봇과학2", funds(89_000, 0, 270_000, 0), Some(359_000)),
            ],
        };
        let checks = verify_department(&dept);
        let v: Vec<&Check> = checks
            .iter()
            .filter(|c| c.kind == "vertical" && c.status == CheckStatus::Error)
            .collect();
        assert_eq!(v.len(), 1, "수익자 하나만 어긋나야 한다");
        assert_eq!(v[0].fund.as_deref(), Some("beneficiary"));
        assert_eq!(v[0].diff, Some(-1_000));
        // 가로는 둘 다 맞는다 (견적 총액도 함께 줄였으므로)
        assert!(checks
            .iter()
            .filter(|c| c.kind == "horizontal")
            .all(|c| c.status == CheckStatus::Ok));
    }

    /// 가로가 어긋나면 그 거래처를 짚어 준다
    #[test]
    fn horizontal_mismatch_is_reported_per_vendor() {
        let dept = DeptAllocation {
            department_id: 1,
            department_name: "바둑".into(),
            settlement: funds(372_000, 12_000, 60_000, 0),
            has_settlement: true,
            // 견적서 총액이 정산 합계보다 10,000 많다
            vendors: vec![vendor(10, "바둑", funds(372_000, 12_000, 60_000, 0), Some(454_000))],
        };
        let checks = verify_department(&dept);
        let h: Vec<&Check> = checks
            .iter()
            .filter(|c| c.kind == "horizontal" && c.status == CheckStatus::Error)
            .collect();
        assert_eq!(h.len(), 1);
        assert_eq!(h[0].diff, Some(-10_000));
        assert_eq!(h[0].vendor_unit_id, Some(10));
    }

    #[test]
    fn missing_allocation_blocks() {
        let mut v = vendor(20, "로봇과학1", Funds::default(), Some(900_000));
        v.missing = true;
        let dept = DeptAllocation {
            department_id: 2,
            department_name: "로봇과학".into(),
            settlement: funds(810_000, 6_200, 443_800, 0),
            has_settlement: true,
            vendors: vec![v, vendor(21, "로봇과학2", Funds::default(), Some(360_000))],
        };
        let checks = verify_department(&dept);
        assert!(checks
            .iter()
            .any(|c| c.kind == "allocation_missing" && c.status == CheckStatus::Error));
    }

    #[test]
    fn missing_quote_and_settlement() {
        let a = DeptAllocation {
            department_id: 3,
            department_name: "체스".into(),
            settlement: funds(268_000, 0, 48_000, 14_000),
            has_settlement: true,
            vendors: vec![vendor(30, "체스", Funds::default(), None)],
        };
        assert!(verify_department(&a).iter().any(|c| c.kind == "missing_quote"));

        let b = DeptAllocation {
            department_id: 4,
            department_name: "신규부서".into(),
            settlement: Funds::default(),
            has_settlement: false,
            vendors: vec![vendor(40, "신규", Funds::default(), Some(100_000))],
        };
        assert!(verify_department(&b).iter().any(|c| c.kind == "missing_settlement"));
    }

    /// 0원 재원은 품의 행을 만들지 않는다 (설계안 14장 14번)
    #[test]
    fn zero_fund_makes_no_row() {
        let entries = vec![
            (10i64, "바둑부 바둑교재(상상바둑) 1종".to_string(), funds(372_000, 12_000, 60_000, 0)),
            (11, "주산암산부 방과후 기초Yap! 상 외 3종".to_string(), funds(40_000, 0, 18_000, 10_000)),
        ];
        let b = rows_for_fund(Fund::Beneficiary, &entries);
        assert_eq!(b.len(), 2);
        assert_eq!(b[0].amount, 372_000);

        // 초과금: 주산암산은 0원이라 행이 없다
        let e = rows_for_fund(Fund::Excess, &entries);
        assert_eq!(e.len(), 1);
        assert_eq!(e[0].vendor_unit_id, 10);

        // 자유수강권: 바둑은 0원이라 행이 없다
        let v = rows_for_fund(Fund::Voucher, &entries);
        assert_eq!(v.len(), 1);
        assert_eq!(v[0].vendor_unit_id, 11);
        assert_eq!(v[0].amount, 10_000);
    }
}
