//! 품의 내용 문구 생성 (설계안 6-6).
//!
//! `품의 표기명 + " " + 대표품목 display_name + " " + ("1종" | "외 N종")`
//!
//! - **품의 표기명은 모든 부서에 붙인다.** 접두를 빼는 예외는 없다 (설계안 14장 11번).
//! - `N` 은 `kind == Item` 인 행만 센다. 할인 행과 금액 0원 행은 세지 않는다.
//! - 품목명은 **원문 그대로**다. 프로그램이 띄어쓰기·축약·맞춤법을 손대지 않는다 (14장 2·12번).

use crate::domain::RowKind;

/// 문구 생성에 필요한 최소 정보
pub struct PhraseRow {
    pub kind: RowKind,
    pub display_name: String,
}

/// 대표품목 후보 = `kind == Item` 인 행들. 차례는 원본 그대로.
pub fn item_rows(rows: &[PhraseRow]) -> Vec<&PhraseRow> {
    rows.iter().filter(|r| r.kind == RowKind::Item).collect()
}

/// 대표품목의 기본값 = 첫 번째 일반 품목
pub fn default_representative(rows: &[PhraseRow]) -> Option<usize> {
    rows.iter().position(|r| r.kind == RowKind::Item)
}

/// 문구를 만든다. 일반 품목이 하나도 없으면 `None`.
pub fn build(phrase_name: &str, rows: &[PhraseRow], representative: Option<usize>) -> Option<String> {
    let items = item_rows(rows);
    if items.is_empty() {
        return None;
    }
    let n = items.len();

    // 대표품목이 지정돼 있고 그것이 일반 품목이면 그것을, 아니면 첫 일반 품목을 쓴다.
    let rep_name = representative
        .and_then(|i| rows.get(i))
        .filter(|r| r.kind == RowKind::Item)
        .map(|r| r.display_name.as_str())
        .unwrap_or(items[0].display_name.as_str());

    let tail = if n == 1 { "1종".to_string() } else { format!("외 {}종", n - 1) };
    Some(format!("{} {} {}", phrase_name.trim(), rep_name.trim(), tail))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn row(kind: RowKind, name: &str) -> PhraseRow {
        PhraseRow { kind, display_name: name.into() }
    }

    /// 사람이 실제로 쓴 문구와 글자까지 같아야 한다 (02 문서 6-1)
    #[test]
    fn matches_real_world_phrases() {
        let rows = vec![
            row(RowKind::Item, "프로보테크닉 교구"),
            row(RowKind::Item, "프로보테크닉 교재"),
        ];
        assert_eq!(
            build("로봇과학부", &rows, None).unwrap(),
            "로봇과학부 프로보테크닉 교구 외 1종"
        );

        let rows = vec![
            row(RowKind::Item, "방과후 기초Yap! 상"),
            row(RowKind::Item, "방과후 기초Yap! 하"),
            row(RowKind::Item, "10급Yap!"),
            row(RowKind::Item, "암산교재"),
        ];
        assert_eq!(
            build("주산암산부", &rows, None).unwrap(),
            "주산암산부 방과후 기초Yap! 상 외 3종"
        );
    }

    /// 품목명은 원문 그대로 — 사람이 줄인 것을 따라 하지 않는다 (14장 2번)
    #[test]
    fn keeps_item_name_verbatim() {
        let rows = vec![row(RowKind::Item, "바둑교재(상상바둑)")];
        assert_eq!(build("바둑부", &rows, None).unwrap(), "바둑부 바둑교재(상상바둑) 1종");
    }

    /// 사용자가 표시명을 고치면 그 값이 쓰인다 (14장 12번)
    #[test]
    fn uses_edited_display_name() {
        let rows = vec![row(RowKind::Item, "드론항공과학 교구세트")];
        assert_eq!(
            build("항공드론부", &rows, None).unwrap(),
            "항공드론부 드론항공과학 교구세트 1종"
        );
    }

    /// 할인 행은 N종에도 대표품목에도 들어가지 않는다 (14장 9번)
    #[test]
    fn discount_excluded() {
        let rows = vec![
            row(RowKind::Item, "교재A"),
            row(RowKind::Item, "교재B"),
            row(RowKind::Adjustment, "할인"),
        ];
        assert_eq!(build("○○부", &rows, None).unwrap(), "○○부 교재A 외 1종");
        assert_eq!(item_rows(&rows).len(), 2);
    }

    /// 금액 0원 행(사은품)도 마찬가지다 (14장 10번)
    #[test]
    fn zero_row_excluded() {
        let rows = vec![
            row(RowKind::Item, "교재A"),
            row(RowKind::Item, "교재B"),
            row(RowKind::Zero, "사은품 노트"),
        ];
        assert_eq!(build("○○부", &rows, None).unwrap(), "○○부 교재A 외 1종");
    }

    /// 할인이 맨 앞에 있어도 대표품목은 첫 '일반 품목' 이다
    #[test]
    fn representative_skips_non_items() {
        let rows = vec![
            row(RowKind::Adjustment, "할인"),
            row(RowKind::Zero, "사은품"),
            row(RowKind::Item, "교재A"),
        ];
        assert_eq!(default_representative(&rows), Some(2));
        assert_eq!(build("○○부", &rows, None).unwrap(), "○○부 교재A 1종");
    }

    #[test]
    fn user_can_pick_representative() {
        let rows = vec![
            row(RowKind::Item, "교재A"),
            row(RowKind::Item, "교재B"),
            row(RowKind::Item, "교재C"),
        ];
        assert_eq!(build("○○부", &rows, Some(1)).unwrap(), "○○부 교재B 외 2종");
        // 일반 품목이 아닌 자리를 가리키면 기본값으로 되돌아간다
        assert_eq!(build("○○부", &rows, Some(99)).unwrap(), "○○부 교재A 외 2종");
    }

    #[test]
    fn no_items_no_phrase() {
        let rows = vec![row(RowKind::Adjustment, "할인"), row(RowKind::Zero, "사은품")];
        assert!(build("○○부", &rows, None).is_none());
    }
}
