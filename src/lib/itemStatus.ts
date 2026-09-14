import type { ItemRow, QuoteSource } from '../ipc/types'

/**
 * 품목 한 줄을 **사람이 다시 봐야 하는가**.
 *
 * 프로그램은 이미 줄마다 신뢰도와 경고를 계산해 두었다(`quote/trust.rs`).
 * 그 값을 화면에서 그대로 쓴다. **여기서 새로 판단하거나 값을 고치지 않는다.**
 */
export function needsLook(it: ItemRow): boolean {
  if (it.confidence === 'low') return true
  return it.warnings.some((w) => w.severity === 'warn' || w.severity === 'error')
}

/**
 * 왜 다시 봐야 하는지 — 사람이 읽는 한 줄들.
 *
 * 경고 문장이 있으면 그 문장을 쓴다. 문장 없이 신뢰도만 낮으면 까닭을 한 줄로 적어 준다.
 */
export function lookReasons(it: ItemRow, source: QuoteSource): string[] {
  const out = it.warnings
    .filter((w) => w.severity === 'warn' || w.severity === 'error')
    .map((w) => w.message)
  if (out.length > 0) return out
  if (it.confidence === 'low') {
    if (it.kind === 'item' && (it.qty === null || it.unitPrice === null || it.amount === null)) {
      return ['수량·단가·금액 중 읽지 못한 값이 있습니다.']
    }
    return [
      source === 'ocr'
        ? '사진에서 제대로 읽히지 않았을 수 있습니다.'
        : '읽은 값이 확실하지 않습니다.',
    ]
  }
  return []
}

/** 다시 봐야 하는 줄이 몇 개인가 */
export function countNeedsLook(items: ItemRow[]): number {
  return items.filter(needsLook).length
}

/**
 * 빈 줄인가 — 손으로 추가했지만 아직 아무것도 넣지 않은 줄.
 * 이런 줄이 남아 있으면 품의 문구의 「N종」이 잘못 세어진다.
 */
export function isBlank(it: ItemRow): boolean {
  return (
    it.displayName.trim() === '' &&
    it.spec.trim() === '' &&
    it.qty === null &&
    it.unitPrice === null &&
    it.amount === null
  )
}
