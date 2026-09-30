/**
 * 작업 이름 규칙.
 *
 * 서버(`domain/work.rs`의 `clean_title`)와 **같은 규칙**을 쓴다.
 * 화면에서 먼저 막아 주되, 실제로 지키는 쪽은 서버다. 둘이 어긋나면
 * 화면에서는 통과했는데 저장이 안 되거나, 이름 없는 작업이 목록에 남는다.
 *
 * `String.prototype.trim()` 과 Rust 의 `str::trim()` 은 둘 다 유니코드 공백을 뗀다.
 * 전각 공백(`　`)도 양쪽에서 똑같이 공백으로 본다.
 */

/** 앞뒤 공백을 뗀 이름. 가운데 공백은 사용자가 친 그대로 둔다. */
export function cleanWorkName(raw: string): string {
  return raw.trim()
}

/** 쓸 수 있는 이름인가 */
export function isValidWorkName(raw: string): boolean {
  return cleanWorkName(raw) !== ''
}

/** 못 쓰는 까닭 한 줄. 쓸 수 있으면 `null`. */
export function workNameError(raw: string): string | null {
  return isValidWorkName(raw) ? null : '작업 이름을 입력해 주세요.'
}

/** 3월부터 새 학년도가 시작한다 */
export function schoolYearOf(when: Date): string {
  const year = when.getMonth() + 1 >= 3 ? when.getFullYear() : when.getFullYear() - 1
  return `${year}학년도`
}

export function monthOf(when: Date): string {
  return `${when.getMonth() + 1}월`
}

/**
 * 새 작업 칸에 미리 채워 두는 이름.
 *
 * **제안일 뿐이다.** 사용자가 지우고 다른 이름을 칠 수 있어야 한다.
 */
export function defaultWorkName(when: Date, kind = '교재비'): string {
  return `${schoolYearOf(when)} ${monthOf(when)} ${kind}`
}
