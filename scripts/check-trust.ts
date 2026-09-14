/**
 * 신뢰 상태 표시 규칙 점검 (브라우저 없이 도는 순수 함수 시험).
 *   npx tsx scripts/check-trust.ts
 */
import { TRUST_LABEL, TRUST_BADGE, SOURCE_LABEL, needsHumanCheck, sourceNote } from '../src/lib/quoteTrust.ts'
import type { QuoteRow } from '../src/ipc/types.ts'

let fail = 0
function eq(got: unknown, want: unknown, what: string) {
  if (JSON.stringify(got) !== JSON.stringify(want)) {
    console.error(`✗ ${what}\n  받음: ${JSON.stringify(got)}\n  기대: ${JSON.stringify(want)}`)
    fail++
  } else {
    console.log(`✓ ${what}`)
  }
}

const base = { parseStatus: 'ok', source: 'structured', trust: 'ok' } as unknown as QuoteRow

eq(TRUST_LABEL.amount_failed, '금액 검증 실패', '금액 검증 실패 이름')
eq(TRUST_LABEL.uncertain, '인식 불확실', '인식 불확실 이름')
eq(TRUST_BADGE.amount_failed, 'bad', '금액 검증 실패는 빨강')
eq(SOURCE_LABEL.ocr, '사진 글자 인식', '사진 경로 이름')

eq(needsHumanCheck(base), false, 'XLSX 는 따로 확인 표시를 하지 않는다')
eq(needsHumanCheck({ ...base, source: 'ocr' }), true, '사진은 사람이 봐야 한다')
eq(needsHumanCheck({ ...base, source: 'pdf_text' }), true, 'PDF 글자도 사람이 봐야 한다')
eq(
  needsHumanCheck({ ...base, source: 'ocr', parseStatus: 'failed' }),
  false,
  '못 읽은 것은 직접 입력 안내가 따로 나간다',
)

eq(sourceNote(base), '', 'XLSX 는 안내가 없다')
eq(
  sourceNote({ ...base, source: 'ocr' }),
  '사진에서 읽었습니다. 품목과 금액을 확인해 주세요.',
  '사진 안내 문구',
)

console.log(fail === 0 ? '\n모두 통과' : `\n${fail}건 실패`)
process.exit(fail === 0 ? 0 : 1)
