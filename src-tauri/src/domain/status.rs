//! P1-8 상태 계산과 생성 차단 (설계안 4-4, 14장 13번).
//!
//! 빨강이 하나라도 있으면 생성이 잠긴다. 푸는 길은 하나뿐이다 —
//! **빨강인 항목마다 사용자가 확인하고 사유를 한 줄 적는다(공란 불가).**
//! "항상 허용" 설정은 없다.

use serde::{Deserialize, Serialize};

/// 화면에 보이는 상태 (나쁜 순서대로 정렬된다)
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Status {
    /// 경고 없음
    Ok,
    /// info 급
    Notice,
    /// warn 급 — 사람이 봐야 하지만 생성을 막지는 않는다
    NeedsCheck,
    /// error 급 — 생성을 막는다
    Blocked,
}

impl Status {
    pub fn label(self) -> &'static str {
        match self {
            Status::Ok => "정상",
            Status::Notice => "주의",
            Status::NeedsCheck => "확인 필요",
            Status::Blocked => "오류",
        }
    }
    pub fn blocks(self) -> bool {
        self == Status::Blocked
    }
}

/// 생성을 막는 항목 하나
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Blocker {
    pub check_id: i64,
    pub kind: String,
    pub label: String,
    pub acknowledged: bool,
    pub ack_reason: String,
}

/// 생성 가능 여부
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GateResult {
    pub can_generate: bool,
    /// 아직 확인되지 않은 빨강 항목
    pub open_blockers: Vec<Blocker>,
    /// 사용자가 사유를 적고 확인한 빨강 항목
    pub acknowledged: Vec<Blocker>,
    pub message: String,
}

/// 빨강 목록을 보고 생성해도 되는지 정한다.
pub fn gate(blockers: Vec<Blocker>) -> GateResult {
    let (acked, open): (Vec<Blocker>, Vec<Blocker>) =
        blockers.into_iter().partition(|b| b.acknowledged && !b.ack_reason.trim().is_empty());

    let can = open.is_empty();
    let message = if can && acked.is_empty() {
        "모든 검증을 통과했습니다.".to_string()
    } else if can {
        format!("확인된 불일치 {}건이 있습니다. 사유가 작업에 남습니다.", acked.len())
    } else {
        format!("확인이 필요한 항목이 {}건 남아 있습니다.", open.len())
    };

    GateResult { can_generate: can, open_blockers: open, acknowledged: acked, message }
}

/// 사유가 비어 있으면 확인으로 치지 않는다 (설계안 14장 13번)
pub fn validate_reason(reason: &str) -> Result<(), &'static str> {
    if reason.trim().is_empty() {
        Err("확인 사유를 적어 주세요. 비워 둘 수 없습니다.")
    } else {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn b(id: i64, acked: bool, reason: &str) -> Blocker {
        Blocker {
            check_id: id,
            kind: "vertical".into(),
            label: "테스트".into(),
            acknowledged: acked,
            ack_reason: reason.into(),
        }
    }

    #[test]
    fn no_blockers_can_generate() {
        let g = gate(vec![]);
        assert!(g.can_generate);
        assert!(g.open_blockers.is_empty());
    }

    #[test]
    fn open_blocker_stops_generation() {
        let g = gate(vec![b(1, false, "")]);
        assert!(!g.can_generate);
        assert_eq!(g.open_blockers.len(), 1);
    }

    /// 사유를 적고 확인하면 그 항목에 한해 통과한다
    #[test]
    fn acknowledged_with_reason_passes() {
        let g = gate(vec![b(1, true, "학부모 환불로 정산이 늦게 반영됨")]);
        assert!(g.can_generate);
        assert_eq!(g.acknowledged.len(), 1);
        assert!(g.message.contains("사유"));
    }

    /// **사유가 비면 확인으로 치지 않는다**
    #[test]
    fn blank_reason_is_not_acknowledgement() {
        let g = gate(vec![b(1, true, "   ")]);
        assert!(!g.can_generate, "공란 사유로는 풀리면 안 된다");
        assert_eq!(g.open_blockers.len(), 1);
        assert!(validate_reason("   ").is_err());
        assert!(validate_reason("사정이 있었음").is_ok());
    }

    /// 확인한 것과 안 한 것이 섞이면 막힌다
    #[test]
    fn partial_acknowledgement_still_blocks() {
        let g = gate(vec![b(1, true, "사유 있음"), b(2, false, "")]);
        assert!(!g.can_generate);
        assert_eq!(g.acknowledged.len(), 1);
        assert_eq!(g.open_blockers.len(), 1);
    }

    #[test]
    fn status_order() {
        assert!(Status::Blocked > Status::NeedsCheck);
        assert!(Status::NeedsCheck > Status::Notice);
        assert!(Status::Notice > Status::Ok);
        assert!(Status::Blocked.blocks());
        assert!(!Status::NeedsCheck.blocks());
    }
}
