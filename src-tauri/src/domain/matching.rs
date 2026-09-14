//! P1-3 파일명 → 거래처 관리 단위 매칭 (설계안 8-1, 14장 1번).
//!
//! 규칙은 "등록된 이름이 파일명 안에 들어 있는가" 뿐이다. **형식을 강제하지 않는다.**
//! `주산암산부 견적서.jpg` · `주산암산 견적서류.jpg` · `주산암산 9월 견적서.jpg` 가 모두 같게 매칭된다.
//!
//! **절대 하지 않는 것 둘**
//!   1. 정산 별칭을 쓰지 않는다 (쓰면 `토탈공예미니어처1 견적서.xlsx` 가 없는 거래처로 붙는다)
//!   2. 숫자 접미사를 떼거나 붙여 거래처를 추측하지 않는다

use serde::{Deserialize, Serialize};

/// 매칭에 쓸 후보. **정산 별칭은 여기에 넣지 않는다.**
#[derive(Debug, Clone)]
pub struct MatchCandidate {
    pub vendor_unit_id: i64,
    pub mgmt_name: String,
    pub department_id: i64,
    pub department_name: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum MatchMethod {
    Auto,
    Manual,
    None,
}

impl MatchMethod {
    pub fn key(self) -> &'static str {
        match self {
            MatchMethod::Auto => "auto",
            MatchMethod::Manual => "manual",
            MatchMethod::None => "none",
        }
    }
    pub fn from_key(s: &str) -> MatchMethod {
        match s {
            "auto" => MatchMethod::Auto,
            "manual" => MatchMethod::Manual,
            _ => MatchMethod::None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MatchResult {
    /// 자동으로 정해진 거래처. 없으면 사용자가 골라야 한다.
    pub vendor_unit_id: Option<i64>,
    pub method: MatchMethod,
    /// 사용자가 고를 후보들 (둘 이상일 때)
    pub candidates: Vec<i64>,
    /// 화면에 보여 줄 설명
    pub note: String,
}

/// 파일명 정규화: 확장자 제거 → 공백·`_`·`-`·괄호 제거 → 전각→반각 → 소문자.
pub fn normalize(name: &str) -> String {
    let stem = match name.rfind('.') {
        Some(i) if i > 0 => &name[..i],
        _ => name,
    };
    stem.chars()
        .filter_map(|c| {
            // 전각 영숫자 → 반각
            let c = if ('！'..='～').contains(&c) {
                char::from_u32(c as u32 - 0xFEE0).unwrap_or(c)
            } else if c == '　' {
                ' '
            } else {
                c
            };
            match c {
                ' ' | '\t' | '_' | '-' | '(' | ')' | '[' | ']' | '{' | '}' | '.' | ',' => None,
                _ => Some(c.to_lowercase().next().unwrap_or(c)),
            }
        })
        .collect()
}

/// 설계안 8-1 의 다섯 단계를 그대로 따른다.
pub fn match_file(file_name: &str, candidates: &[MatchCandidate]) -> MatchResult {
    let hay = normalize(file_name);

    // 1) 거래처 관리명 포함 검사 — **긴 이름부터**.
    //    `로봇과학1견적서` 는 `로봇과학1` 에 맞고 더 짧은 `로봇과학` 에는 맞추지 않는다.
    let mut by_mgmt: Vec<&MatchCandidate> = candidates
        .iter()
        .filter(|c| {
            let n = normalize(&c.mgmt_name);
            !n.is_empty() && hay.contains(&n)
        })
        .collect();
    by_mgmt.sort_by_key(|c| std::cmp::Reverse(normalize(&c.mgmt_name).chars().count()));

    if let Some(best) = by_mgmt.first() {
        let best_len = normalize(&best.mgmt_name).chars().count();
        let tied: Vec<&&MatchCandidate> = by_mgmt
            .iter()
            .filter(|c| normalize(&c.mgmt_name).chars().count() == best_len)
            .collect();
        if tied.len() == 1 {
            return MatchResult {
                vendor_unit_id: Some(best.vendor_unit_id),
                method: MatchMethod::Auto,
                candidates: vec![best.vendor_unit_id],
                note: format!("파일명에 거래처 관리명 '{}' 이 있습니다.", best.mgmt_name),
            };
        }
        // 같은 길이의 이름 여럿이 걸렸다 — 추측하지 않는다
        return MatchResult {
            vendor_unit_id: None,
            method: MatchMethod::None,
            candidates: tied.iter().map(|c| c.vendor_unit_id).collect(),
            note: "파일명에 맞는 거래처 관리명이 여러 개입니다. 직접 골라 주세요.".into(),
        };
    }

    // 2) 품의 부서명 포함 검사
    let mut dept_hits: Vec<(i64, String)> = Vec::new();
    for c in candidates {
        let n = normalize(&c.department_name);
        if !n.is_empty() && hay.contains(&n) && !dept_hits.iter().any(|(id, _)| *id == c.department_id)
        {
            dept_hits.push((c.department_id, c.department_name.clone()));
        }
    }
    // 부서명도 긴 것부터 (예: '통합과학' 과 '과학' 이 둘 다 있을 때)
    dept_hits.sort_by_key(|(_, name)| std::cmp::Reverse(normalize(name).chars().count()));

    if let Some((dept_id, dept_name)) = dept_hits.first().cloned() {
        let longest = normalize(&dept_name).chars().count();
        let tied: Vec<&(i64, String)> = dept_hits
            .iter()
            .filter(|(_, n)| normalize(n).chars().count() == longest)
            .collect();
        if tied.len() > 1 {
            let vendors: Vec<i64> = candidates
                .iter()
                .filter(|c| tied.iter().any(|(id, _)| *id == c.department_id))
                .map(|c| c.vendor_unit_id)
                .collect();
            return MatchResult {
                vendor_unit_id: None,
                method: MatchMethod::None,
                candidates: vendors,
                note: "파일명에 맞는 부서가 여러 개입니다. 직접 골라 주세요.".into(),
            };
        }

        let in_dept: Vec<&MatchCandidate> =
            candidates.iter().filter(|c| c.department_id == dept_id).collect();

        // 3) 거래처가 하나면 자동 매칭
        if in_dept.len() == 1 {
            return MatchResult {
                vendor_unit_id: Some(in_dept[0].vendor_unit_id),
                method: MatchMethod::Auto,
                candidates: vec![in_dept[0].vendor_unit_id],
                note: format!("'{dept_name}' 부서에 거래처가 하나뿐이라 자동으로 정했습니다."),
            };
        }
        // 4) 둘 이상이면 **추측하지 않는다**
        return MatchResult {
            vendor_unit_id: None,
            method: MatchMethod::None,
            candidates: in_dept.iter().map(|c| c.vendor_unit_id).collect(),
            note: format!("{dept_name} 부서는 거래처가 여러 개입니다. 거래처를 골라 주세요."),
        };
    }

    // 5) 부서명도 못 찾음
    MatchResult {
        vendor_unit_id: None,
        method: MatchMethod::None,
        candidates: Vec::new(),
        note: "파일명에서 등록된 부서명이나 거래처 관리명을 찾지 못했습니다. 직접 골라 주세요.".into(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cand(id: i64, mgmt: &str, dept_id: i64, dept: &str) -> MatchCandidate {
        MatchCandidate {
            vendor_unit_id: id,
            mgmt_name: mgmt.into(),
            department_id: dept_id,
            department_name: dept.into(),
        }
    }

    /// 설계안 14장 1번의 예시들 — 형식이 달라도 모두 같게 매칭된다
    #[test]
    fn filename_shape_does_not_matter() {
        let c = vec![cand(1, "주산암산", 10, "주산암산")];
        for name in [
            "주산암산부 견적서.jpg",
            "주산암산 견적서.jpg",
            "주산암산 견적서류.jpg",
            "주산암산 9월 견적서.jpg",
            "2026 주산암산_견적(최종).xlsx",
        ] {
            let r = match_file(name, &c);
            assert_eq!(r.vendor_unit_id, Some(1), "{name}");
            assert_eq!(r.method, MatchMethod::Auto, "{name}");
        }
    }

    #[test]
    fn longer_mgmt_name_wins() {
        let c = vec![
            cand(1, "로봇과학1", 10, "로봇과학"),
            cand(2, "로봇과학2", 10, "로봇과학"),
        ];
        let r = match_file("로봇과학1 견적서.hwp", &c);
        assert_eq!(r.vendor_unit_id, Some(1));
        assert_eq!(r.method, MatchMethod::Auto);

        let r = match_file("로봇과학2 견적서.xlsx", &c);
        assert_eq!(r.vendor_unit_id, Some(2));
    }

    /// 거래처가 여럿인 부서에 부서명만 있으면 **추측하지 않는다**
    #[test]
    fn never_guesses_between_vendors() {
        let c = vec![
            cand(1, "로봇과학1", 10, "로봇과학"),
            cand(2, "로봇과학2", 10, "로봇과학"),
        ];
        let r = match_file("로봇과학 견적서.hwp", &c);
        assert_eq!(r.vendor_unit_id, None);
        assert_eq!(r.method, MatchMethod::None);
        assert_eq!(r.candidates, vec![1, 2]);
        assert!(r.note.contains("거래처가 여러 개"), "{}", r.note);
    }

    #[test]
    fn single_vendor_department_auto_matches_by_dept_name() {
        // 관리명이 부서명과 다른 경우에도 부서명으로 찾아 자동 매칭된다
        let c = vec![cand(7, "바둑거래처A", 20, "바둑")];
        let r = match_file("바둑 견적서.xlsx", &c);
        assert_eq!(r.vendor_unit_id, Some(7));
        assert_eq!(r.method, MatchMethod::Auto);
    }

    #[test]
    fn no_match_at_all() {
        let c = vec![cand(1, "바둑", 20, "바둑")];
        let r = match_file("무슨무슨 견적서.xlsx", &c);
        assert_eq!(r.vendor_unit_id, None);
        assert!(r.candidates.is_empty());
    }

    /// 정산 별칭은 후보에 넣지 않으므로 맞을 수가 없다 (설계안 8-1 주의)
    #[test]
    fn settlement_alias_is_not_a_candidate() {
        // '토탈공예미니어처1' 은 정산 별칭일 뿐 거래처가 아니다.
        // 후보에는 부서(토탈공예미니어처)와 그 거래처 하나만 있다.
        let c = vec![cand(5, "토탈공예", 30, "토탈공예미니어처")];
        let r = match_file("토탈공예미니어처1 견적서.xlsx", &c);
        // 부서명이 파일명에 들어 있고 거래처가 하나뿐이므로 부서로 자동 매칭된다.
        // '1' 은 해석하지 않는다 — 없는 거래처를 만들어 내지 않는다.
        assert_eq!(r.vendor_unit_id, Some(5));
        assert_eq!(r.method, MatchMethod::Auto);
    }

    #[test]
    fn normalize_rules() {
        assert_eq!(normalize("주산암산 견적서.jpg"), "주산암산견적서");
        assert_eq!(normalize("로봇과학1_견적서(최종).hwp"), "로봇과학1견적서최종");
        assert_eq!(normalize("ABC-123.xlsx"), "abc123");
        assert_eq!(normalize("ＡＢＣ１２３.xlsx"), "abc123");
        // 확장자가 없는 이름도 다뤄야 한다
        assert_eq!(normalize("바둑"), "바둑");
    }

    #[test]
    fn department_name_longer_wins() {
        let c = vec![
            cand(1, "과학거래처", 10, "과학"),
            cand(2, "통합과학거래처", 20, "통합과학"),
        ];
        let r = match_file("통합과학 견적서.xlsx", &c);
        assert_eq!(r.vendor_unit_id, Some(2), "더 긴 부서명이 이겨야 한다");
    }
}
