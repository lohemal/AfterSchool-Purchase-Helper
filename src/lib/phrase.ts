/**
 * 품의 내용 문구 미리보기 (설계안 6-6).
 * Rust 가 실제 값을 만들지만, 화면에서 즉시 보여 주려면 같은 규칙이 필요하다.
 * **두 곳의 규칙이 어긋나지 않게 시험으로 묶어 둔다** (`scripts/check-phrase.ts`).
 */

export type RowKind = 'item' | 'adjustment' | 'zero'

export interface PhraseRow {
  kind: RowKind
  displayName: string
}

/** `kind === 'item'` 인 행만 센다. 할인 행과 0원 행은 빠진다. */
export function itemCount(rows: PhraseRow[]): number {
  return rows.filter((r) => r.kind === 'item').length
}

/** 대표품목 기본값 = 첫 일반 품목의 차례 */
export function defaultRepresentative(rows: PhraseRow[]): number {
  return rows.findIndex((r) => r.kind === 'item')
}

/**
 * `품의 표기명 + 대표품목 + ("1종" | "외 N종")`
 * 품의 표기명은 **모든 부서에 붙는다**. 품목명은 원문 그대로 쓴다.
 */
export function buildPhrase(
  phraseName: string,
  rows: PhraseRow[],
  representative?: number,
): string | null {
  const items = rows.filter((r) => r.kind === 'item')
  if (items.length === 0) return null

  const picked =
    representative !== undefined && rows[representative]?.kind === 'item'
      ? rows[representative]
      : items[0]

  const tail = items.length === 1 ? '1종' : `외 ${items.length - 1}종`
  return `${phraseName.trim()} ${picked.displayName.trim()} ${tail}`
}

/** 품의 표기명 기본값 */
export function defaultPhraseName(displayName: string): string {
  return `${displayName.trim()}부`
}
