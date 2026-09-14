//! 화면과 Rust 가 함께 쓰는 값들. 이름은 설계안 2장의 용어를 그대로 따른다.

pub mod allocation;
pub mod matching;
pub mod phrase;
pub mod setup;
pub mod status;
pub mod work;

use serde::{Deserialize, Serialize};

/// 재원 네 가지. 최종 파일 네 개와 1:1 이다.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Fund {
    Beneficiary,
    Excess,
    Subsidy,
    Voucher,
}

pub const ALL_FUNDS: [Fund; 4] =
    [Fund::Beneficiary, Fund::Excess, Fund::Subsidy, Fund::Voucher];

impl Fund {
    pub fn key(self) -> &'static str {
        match self {
            Fund::Beneficiary => "beneficiary",
            Fund::Excess => "excess",
            Fund::Subsidy => "subsidy",
            Fund::Voucher => "voucher",
        }
    }
    /// 화면과 파일 이름에 쓰는 한국어 이름
    pub fn label(self) -> &'static str {
        match self {
            Fund::Beneficiary => "수익자",
            Fund::Excess => "초과금",
            Fund::Subsidy => "지원금",
            Fund::Voucher => "자유수강권",
        }
    }
    pub fn from_key(s: &str) -> Option<Fund> {
        ALL_FUNDS.into_iter().find(|f| f.key() == s)
    }
}

/// 재원 네 칸 묶음. 금액은 원 단위 정수다.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Funds {
    pub beneficiary: i64,
    pub excess: i64,
    pub subsidy: i64,
    pub voucher: i64,
}

impl Funds {
    pub fn get(&self, f: Fund) -> i64 {
        match f {
            Fund::Beneficiary => self.beneficiary,
            Fund::Excess => self.excess,
            Fund::Subsidy => self.subsidy,
            Fund::Voucher => self.voucher,
        }
    }
    pub fn set(&mut self, f: Fund, v: i64) {
        match f {
            Fund::Beneficiary => self.beneficiary = v,
            Fund::Excess => self.excess = v,
            Fund::Subsidy => self.subsidy = v,
            Fund::Voucher => self.voucher = v,
        }
    }
    pub fn total(&self) -> i64 {
        self.beneficiary + self.excess + self.subsidy + self.voucher
    }
    pub fn add(&mut self, other: &Funds) {
        self.beneficiary += other.beneficiary;
        self.excess += other.excess;
        self.subsidy += other.subsidy;
        self.voucher += other.voucher;
    }
}

/// 견적서 행의 종류 (설계안 6-2). 합계·빈 행은 저장하지 않으므로 셋뿐이다.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum RowKind {
    /// 일반 품목 — N종·대표품목·총액에 모두 들어간다
    Item,
    /// 할인·금액 조정 — N종·대표품목에서 빼고 총액에는 넣는다
    Adjustment,
    /// 금액 0원 행(사은품 등) — 전부에서 빼되 참고용으로 보여 준다
    Zero,
}

impl RowKind {
    pub fn key(self) -> &'static str {
        match self {
            RowKind::Item => "item",
            RowKind::Adjustment => "adjustment",
            RowKind::Zero => "zero",
        }
    }
    pub fn from_key(s: &str) -> Option<RowKind> {
        match s {
            "item" => Some(RowKind::Item),
            "adjustment" => Some(RowKind::Adjustment),
            "zero" => Some(RowKind::Zero),
            _ => None,
        }
    }
}

/// 할인 행의 금액이 총액에 어떤 부호로 들어가는가.
/// **원문 값(raw_amount)은 어느 경우에도 바뀌지 않는다.** 계산에만 쓰는 값이다.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum SignEffect {
    /// 원문 부호 그대로 더한다
    AsWritten,
    /// 양수로 적혀 있지만 빼는 값이다
    Subtract,
    /// 표기 총액이 없거나 어느 쪽도 안 맞는다 → 사용자가 고른다
    Unknown,
}

impl SignEffect {
    pub fn key(self) -> &'static str {
        match self {
            SignEffect::AsWritten => "as_written",
            SignEffect::Subtract => "subtract",
            SignEffect::Unknown => "unknown",
        }
    }
    pub fn from_key(s: &str) -> Option<SignEffect> {
        match s {
            "as_written" => Some(SignEffect::AsWritten),
            "subtract" => Some(SignEffect::Subtract),
            "unknown" => Some(SignEffect::Unknown),
            _ => None,
        }
    }
    /// 원문 금액에 이 규칙을 적용한 값
    pub fn apply(self, raw_amount: i64) -> i64 {
        match self {
            SignEffect::AsWritten => raw_amount,
            SignEffect::Subtract => -raw_amount.abs(),
            // 모를 때는 계산에 넣지 않는다 (추정하지 않는다)
            SignEffect::Unknown => 0,
        }
    }
}

/// 정산과 비교할 값을 어디서 가져오는가
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum CompareBasis {
    /// 견적서에 적힌 합계금액(공급가액+세액) — 기본값
    Grand,
    /// 견적서에 적힌 공급가액
    Supply,
    /// 프로그램이 더한 값 (표기 합계를 못 읽었을 때)
    ComputedTotal,
}

impl CompareBasis {
    pub fn key(self) -> &'static str {
        match self {
            CompareBasis::Grand => "grand",
            CompareBasis::Supply => "supply",
            CompareBasis::ComputedTotal => "computed_total",
        }
    }
    pub fn from_key(s: &str) -> Option<CompareBasis> {
        match s {
            "grand" => Some(CompareBasis::Grand),
            "supply" => Some(CompareBasis::Supply),
            "computed_total" => Some(CompareBasis::ComputedTotal),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Confidence {
    High,
    Medium,
    Low,
}

impl Confidence {
    pub fn key(self) -> &'static str {
        match self {
            Confidence::High => "high",
            Confidence::Medium => "medium",
            Confidence::Low => "low",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Severity {
    Info,
    Warn,
    Error,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Warning {
    pub code: String,
    pub message: String,
    pub severity: Severity,
}

impl Warning {
    pub fn info(code: &str, msg: impl Into<String>) -> Self {
        Self { code: code.into(), message: msg.into(), severity: Severity::Info }
    }
    pub fn warn(code: &str, msg: impl Into<String>) -> Self {
        Self { code: code.into(), message: msg.into(), severity: Severity::Warn }
    }
    pub fn error(code: &str, msg: impl Into<String>) -> Self {
        Self { code: code.into(), message: msg.into(), severity: Severity::Error }
    }
}

/// 금액을 사람이 읽는 꼴로 (`1234567` → `1,234,567`)
pub fn comma(n: i64) -> String {
    let neg = n < 0;
    let s = n.abs().to_string();
    let mut out = String::new();
    for (i, c) in s.chars().enumerate() {
        if i > 0 && (s.len() - i) % 3 == 0 {
            out.push(',');
        }
        out.push(c);
    }
    if neg {
        format!("-{out}")
    } else {
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn funds_math() {
        let mut f = Funds { beneficiary: 372_000, excess: 12_000, subsidy: 60_000, voucher: 0 };
        assert_eq!(f.total(), 444_000);
        assert_eq!(f.get(Fund::Excess), 12_000);
        f.set(Fund::Voucher, 1_000);
        assert_eq!(f.total(), 445_000);
        let mut g = Funds::default();
        g.add(&f);
        g.add(&f);
        assert_eq!(g.total(), 890_000);
    }

    #[test]
    fn sign_effect_never_guesses() {
        assert_eq!(SignEffect::AsWritten.apply(-30_000), -30_000);
        assert_eq!(SignEffect::AsWritten.apply(30_000), 30_000);
        assert_eq!(SignEffect::Subtract.apply(30_000), -30_000);
        assert_eq!(SignEffect::Subtract.apply(-30_000), -30_000);
        // 모를 때는 0 으로 두고 경고한다 — 추정하지 않는다
        assert_eq!(SignEffect::Unknown.apply(30_000), 0);
    }

    #[test]
    fn comma_format() {
        assert_eq!(comma(0), "0");
        assert_eq!(comma(1), "1");
        assert_eq!(comma(444_000), "444,000");
        assert_eq!(comma(10_005_700), "10,005,700");
        assert_eq!(comma(-30_000), "-30,000");
    }

    #[test]
    fn key_round_trip() {
        for f in ALL_FUNDS {
            assert_eq!(Fund::from_key(f.key()), Some(f));
        }
        for k in [RowKind::Item, RowKind::Adjustment, RowKind::Zero] {
            assert_eq!(RowKind::from_key(k.key()), Some(k));
        }
        for s in [SignEffect::AsWritten, SignEffect::Subtract, SignEffect::Unknown] {
            assert_eq!(SignEffect::from_key(s.key()), Some(s));
        }
        for c in [CompareBasis::Grand, CompareBasis::Supply, CompareBasis::ComputedTotal] {
            assert_eq!(CompareBasis::from_key(c.key()), Some(c));
        }
    }
}
