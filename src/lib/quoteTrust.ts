import type { QuoteRow, QuoteSource, QuoteTrust } from '../ipc/types'

/** 한 줄 신뢰 상태를 사람 말로. Rust `quote/trust.rs` 의 이름과 1:1 이다. */
export const TRUST_LABEL: Record<QuoteTrust, string> = {
  '': '',
  ok: '정상',
  needs_check: '확인 필요',
  uncertain: '인식 불확실',
  amount_failed: '금액 검증 실패',
}

/** 상태마다 쓰는 배지 색 (styles.css 의 badge 종류) */
export const TRUST_BADGE: Record<QuoteTrust, string> = {
  '': '',
  ok: 'ok',
  needs_check: 'check',
  uncertain: 'check',
  amount_failed: 'bad',
}

/** 어느 경로로 읽었는지 */
export const SOURCE_LABEL: Record<QuoteSource, string> = {
  structured: '파일에서 그대로',
  pdf_text: 'PDF 글자',
  ocr: '사진 글자 인식',
}

/**
 * 사진·PDF 에서 읽은 견적서는 **사람이 한 번 봐야** 한다.
 * 프로그램이 대신 확정하지 않는다.
 */
export function needsHumanCheck(q: QuoteRow): boolean {
  return q.parseStatus === 'ok' && q.source !== 'structured'
}

/** 표에 보여 줄 한 줄 안내 (없으면 빈 문자열) */
export function sourceNote(q: QuoteRow): string {
  if (q.parseStatus !== 'ok') return ''
  if (q.source === 'ocr') return '사진에서 읽었습니다. 품목과 금액을 확인해 주세요.'
  if (q.source === 'pdf_text') return 'PDF 안의 글자로 읽었습니다. 값을 확인해 주세요.'
  return ''
}
