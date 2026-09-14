/** 금액 표시와 입력. 회계 자료이므로 **반올림·추정을 하지 않는다.** */

export function comma(n: number | null | undefined): string {
  if (n === null || n === undefined) return ''
  const neg = n < 0
  const s = Math.abs(Math.trunc(n)).toString()
  let out = ''
  for (let i = 0; i < s.length; i++) {
    if (i > 0 && (s.length - i) % 3 === 0) out += ','
    out += s[i]
  }
  return neg ? `-${out}` : out
}

/**
 * 사용자가 친 금액을 정수로. 숫자가 없으면 null.
 * 괄호는 **안이 숫자뿐일 때만** 음수로 본다 (P0-5 에서 걸린 함정).
 */
export function parseMoney(input: string): number | null {
  const t = input.trim()
  if (t === '') return null
  const inner = t.startsWith('(') && t.endsWith(')') ? t.slice(1, -1) : null
  const parenNegative = inner !== null && inner !== '' && /^[\d,.\s]+$/.test(inner)
  const negative = t.startsWith('-') || t.startsWith('△') || t.startsWith('▲') || parenNegative
  const digits = t.replace(/[^\d]/g, '')
  if (digits === '') return null
  const n = Number(digits)
  if (!Number.isSafeInteger(n)) return null
  return negative ? -n : n
}

/** 차액을 사람이 읽는 문장으로 */
export function diffText(diff: number | null | undefined): string {
  if (diff === null || diff === undefined || diff === 0) return '일치'
  return diff > 0 ? `${comma(diff)}원 많음` : `${comma(-diff)}원 모자람`
}
