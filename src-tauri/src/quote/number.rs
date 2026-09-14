//! 숫자 정규화 (설계안 6-1 5번).
//!
//! `76,500원` `₩76,500` `10개` `1 set` `-30,000` `△30,000` `(30,000)` → 정수(**부호 보존**).
//!
//! **괄호 예외 (P0-5 에서 실제로 걸렸다)**: 한글 견적서는 합계를 `(￦ 900,000 )` 로 적는다.
//! 이건 음수가 아니다. 그래서 **괄호 안이 숫자·쉼표·마침표·공백뿐일 때만** 회계식 음수로 본다.

/// 문자열에서 정수를 뽑는다. 숫자가 하나도 없으면 `None`.
pub fn parse(s: &str) -> Option<i64> {
    let t = s.trim();
    if t.is_empty() {
        return None;
    }

    let inner_is_plain_number = t.starts_with('(')
        && t.ends_with(')')
        && t.chars().count() >= 2
        && {
            let inner: String = t.chars().skip(1).take(t.chars().count() - 2).collect();
            !inner.is_empty()
                && inner
                    .chars()
                    .all(|c| c.is_ascii_digit() || c == ',' || c == '.' || c.is_whitespace())
        };

    let negative = t.starts_with('-')
        || t.starts_with('△')
        || t.starts_with('▲')
        || t.starts_with('▵')
        || inner_is_plain_number;

    let digits: String = t.chars().filter(|c| c.is_ascii_digit()).collect();
    if digits.is_empty() {
        return None;
    }
    // 아주 긴 숫자는 금액이 아니다 (사업자번호·전화번호 등)
    let n: i64 = digits.parse().ok()?;
    Some(if negative { -n } else { n })
}

/// 숫자처럼 보이는가 (열 역할 추정에 쓴다)
pub fn looks_numeric(s: &str) -> bool {
    let t = s.trim();
    if t.is_empty() {
        return false;
    }
    let digits = t.chars().filter(|c| c.is_ascii_digit()).count();
    let letters = t
        .chars()
        .filter(|c| c.is_alphabetic() && !c.is_ascii_digit())
        .count();
    digits > 0 && digits >= letters
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn basic_forms() {
        assert_eq!(parse("76,500원"), Some(76_500));
        assert_eq!(parse("₩76,500"), Some(76_500));
        assert_eq!(parse("￦900,000"), Some(900_000));
        assert_eq!(parse("10개"), Some(10));
        assert_eq!(parse("1 set"), Some(1));
        assert_eq!(parse("37"), Some(37));
        assert_eq!(parse("  444,000  "), Some(444_000));
    }

    #[test]
    fn negative_forms() {
        assert_eq!(parse("-30,000"), Some(-30_000));
        assert_eq!(parse("△30,000"), Some(-30_000));
        assert_eq!(parse("▲30,000"), Some(-30_000));
        assert_eq!(parse("(30,000)"), Some(-30_000));
        assert_eq!(parse("( 30,000 )"), Some(-30_000));
    }

    /// P0-5 에서 실제로 시험을 실패시킨 함정
    #[test]
    fn paren_with_currency_is_not_negative() {
        assert_eq!(parse("(￦ 900,000 )"), Some(900_000));
        assert_eq!(parse("(₩900,000)"), Some(900_000));
        assert_eq!(parse("(900,000원)"), Some(900_000));
        assert_eq!(parse("(합계 900,000)"), Some(900_000));
    }

    #[test]
    fn no_digits() {
        assert_eq!(parse(""), None);
        assert_eq!(parse("   "), None);
        assert_eq!(parse("도서는 면세임"), None);
        assert_eq!(parse("품명"), None);
        assert_eq!(parse("()"), None);
    }

    #[test]
    fn numeric_look() {
        assert!(looks_numeric("444,000"));
        assert!(looks_numeric("10개"));
        assert!(!looks_numeric("바둑교재"));
        assert!(!looks_numeric(""));
        assert!(!looks_numeric("도서는 면세임"));
    }
}
