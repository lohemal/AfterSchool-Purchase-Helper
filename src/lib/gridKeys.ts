/**
 * 품목 표에서 **Enter 로 같은 칸의 다음 줄**로 내려가기.
 *
 * 값을 여러 줄 고칠 때 마우스를 다시 잡지 않아도 되게 한다.
 * 표의 한 줄에는 글자 칸이 늘 다섯 개다: 물품명 · 규격 · 수량 · 단가 · 금액.
 * 그래서 다음 줄 같은 칸은 **다섯 칸 뒤**다.
 */
export const FIELDS_PER_ROW = 5

/**
 * 지금 칸(`current`)에서 위/아래 줄의 같은 칸 번호.
 * 표 밖으로 나가면 `null` — 그 자리에 그대로 둔다.
 */
export function nextFieldIndex(
  current: number,
  total: number,
  up: boolean,
  perRow: number = FIELDS_PER_ROW,
): number | null {
  if (current < 0 || current >= total) return null
  const next = current + (up ? -perRow : perRow)
  if (next < 0 || next >= total) return null
  return next
}
