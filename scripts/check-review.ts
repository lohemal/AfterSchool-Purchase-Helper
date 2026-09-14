/**
 * 추출 검토 화면의 판단 규칙 시험 (브라우저 없이 도는 순수 함수).
 *   npm run check:review
 *
 * 여기서 보는 것: **프로그램이 계산해 둔 신뢰도·경고를 그대로 쓰는가.**
 * 화면이 스스로 새로 판단하거나 값을 고치면 안 된다.
 */
import { needsLook, lookReasons, countNeedsLook, isBlank } from '../src/lib/itemStatus.ts'
import { nextFieldIndex, FIELDS_PER_ROW } from '../src/lib/gridKeys.ts'
import type { ItemRow, Warning } from '../src/ipc/types.ts'

let fail = 0
function eq(got: unknown, want: unknown, what: string) {
  if (JSON.stringify(got) !== JSON.stringify(want)) {
    console.error(`✗ ${what}\n  받음: ${JSON.stringify(got)}\n  기대: ${JSON.stringify(want)}`)
    fail++
  } else {
    console.log(`✓ ${what}`)
  }
}

function item(patch: Partial<ItemRow> = {}): ItemRow {
  return {
    id: 1,
    rowNo: 0,
    kind: 'item',
    signEffect: null,
    displayName: '교재A',
    spec: '권',
    qty: 3,
    unitPrice: 10000,
    amount: 30000,
    rawName: '교재A',
    rawSpec: '권',
    rawQty: '3',
    rawUnitPrice: '10,000',
    rawAmount: '30,000',
    edited: false,
    cellRef: '사진',
    confidence: 'medium',
    warnings: [],
    ...patch,
  }
}
const warn = (message: string): Warning => ({ code: 'X', message, severity: 'warn' })

// --- 어떤 줄을 다시 봐야 하는가 ---
eq(needsLook(item()), false, '검증을 통과한 줄은 그대로 둔다')
eq(needsLook(item({ confidence: 'low' })), true, '신뢰도가 낮으면 다시 본다')
eq(needsLook(item({ warnings: [warn('수량×단가와 금액이 맞지 않습니다.')] })), true, '경고가 있으면 다시 본다')
eq(
  needsLook(item({ warnings: [{ code: 'X', message: '절사로 보입니다', severity: 'info' }] })),
  false,
  '안내(info)만 있는 줄은 다시 보라고 하지 않는다',
)
eq(needsLook(item({ confidence: 'high' })), false, 'XLSX 에서 온 줄은 조용하다')

// --- 까닭은 프로그램이 만든 문장을 그대로 쓴다 ---
eq(
  lookReasons(item({ warnings: [warn('수량×단가 30000 과 금액 80000 이 맞지 않습니다.')] }), 'ocr'),
  ['수량×단가 30000 과 금액 80000 이 맞지 않습니다.'],
  '경고 문장을 그대로 보여 준다 (새로 만들지 않는다)',
)
eq(
  lookReasons(item({ confidence: 'low', unitPrice: null }), 'ocr'),
  ['수량·단가·금액 중 읽지 못한 값이 있습니다.'],
  '못 읽은 값이 있으면 그렇게 말한다',
)
eq(
  lookReasons(item({ confidence: 'low' }), 'ocr'),
  ['사진에서 제대로 읽히지 않았을 수 있습니다.'],
  '사진이면 사진이라고 말한다',
)
eq(lookReasons(item(), 'ocr'), [], '멀쩡한 줄에는 아무 말도 붙이지 않는다')

eq(
  countNeedsLook([item(), item({ id: 2, confidence: 'low' }), item({ id: 3, warnings: [warn('x')] })]),
  2,
  '다시 볼 줄의 개수',
)

// --- 빈 줄 ---
eq(
  isBlank(item({ displayName: '', spec: '', qty: null, unitPrice: null, amount: null })),
  true,
  '손으로 더하고 안 채운 줄',
)
eq(isBlank(item()), false, '값이 있는 줄은 빈 줄이 아니다')
eq(isBlank(item({ displayName: '', spec: '', qty: null, unitPrice: null, amount: 0 })), false, '0원도 값이다')

// --- Enter 로 같은 칸 다음 줄 ---
eq(FIELDS_PER_ROW, 5, '한 줄의 글자 칸은 물품명·규격·수량·단가·금액 다섯 개')
eq(nextFieldIndex(2, 15, false), 7, '수량 칸에서 Enter → 다음 줄 수량 칸')
eq(nextFieldIndex(7, 15, true), 2, 'Shift+Enter → 윗줄 같은 칸')
eq(nextFieldIndex(12, 15, false), null, '마지막 줄에서는 움직이지 않는다')
eq(nextFieldIndex(1, 15, true), null, '첫 줄에서 위로는 움직이지 않는다')
eq(nextFieldIndex(-1, 15, false), null, '표 밖이면 움직이지 않는다')

console.log(fail === 0 ? '\ncheck-review: 통과' : `\n${fail}건 실패`)
process.exit(fail === 0 ? 0 : 1)
