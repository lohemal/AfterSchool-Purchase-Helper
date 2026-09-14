/**
 * 원본 보기 상태 규칙 시험 (브라우저 없이 도는 순수 함수).
 *   npm run check:preview
 *
 * 요지: **"불러오는 중…" 에서 갇히는 자리가 없어야 한다.**
 * 끝은 언제나 셋 중 하나다 — 그림 표시 / 안내 + 원본 파일 열기 / 실패 + 원본 파일 열기.
 */
import {
  hasPicture,
  previewMessage,
  shouldOfferOpenFile,
  PREVIEW_TIMEOUT_MS,
  type PreviewState,
} from '../src/lib/preview.ts'
import { withTimeout, isTimeout, TimeoutError } from '../src/lib/timeout.ts'
import type { QuotePreview } from '../src/ipc/types.ts'

let fail = 0
function eq(got: unknown, want: unknown, what: string) {
  if (JSON.stringify(got) !== JSON.stringify(want)) {
    console.error(`✗ ${what}\n  받음: ${JSON.stringify(got)}\n  기대: ${JSON.stringify(want)}`)
    fail++
  } else {
    console.log(`✓ ${what}`)
  }
}
function ok(cond: boolean, what: string) {
  eq(cond, true, what)
}

const withPages: QuotePreview = {
  pages: [{ label: '1쪽', mime: 'image/jpeg', base64: 'AAAA', width: 800, height: 1100 }],
  note: '',
}
const noPages: QuotePreview = { pages: [], note: '글자로 된 PDF 라 미리 보여 줄 그림이 없습니다.' }

const idle: PreviewState = { kind: 'idle' }
const loading: PreviewState = { kind: 'loading' }
const ready: PreviewState = { kind: 'ready', data: withPages }
const readyEmpty: PreviewState = { kind: 'ready', data: noPages }
const failed: PreviewState = { kind: 'failed', message: '원본을 불러오는 데 시간이 너무 오래 걸립니다.' }

// --- 셋 중 하나로 끝난다 ---
ok(hasPicture(ready), 'A. 그림이 있으면 보여 준다')
ok(!hasPicture(readyEmpty), '그림이 없는 형식은 그림을 그리지 않는다')
ok(!hasPicture(failed), '실패했으면 그림이 없다')
ok(!hasPicture(loading), '불러오는 중에는 그림이 없다')

eq(previewMessage(readyEmpty), noPages.note, 'B. 그림이 없으면 까닭을 말한다')
ok(shouldOfferOpenFile(readyEmpty), 'B. 그림이 없으면 원본 파일 열기를 권한다')

ok(previewMessage(failed).includes('원본 파일 열기'), 'C. 실패하면 다음 길을 알려 준다')
ok(shouldOfferOpenFile(failed), 'C. 실패하면 원본 파일 열기를 권한다')

// --- 어떤 상태에서도 사람이 볼 말이 있다 (그림이 있을 때만 예외) ---
for (const s of [loading, readyEmpty, failed]) {
  ok(previewMessage(s).length > 0, `${s.kind}: 화면에 할 말이 있다`)
}
eq(previewMessage(ready), '', '그림이 있으면 말 대신 그림이 답이다')
eq(previewMessage(idle), '', '누르기 전에는 아무 말도 없다')

ok(!shouldOfferOpenFile(ready), '그림이 잘 나왔으면 굳이 권하지 않는다')
ok(!shouldOfferOpenFile(loading), '불러오는 중에는 권하지 않는다')

// --- 시간 제한 ---
eq(PREVIEW_TIMEOUT_MS >= 5000 && PREVIEW_TIMEOUT_MS <= 60000, true, '제한 시간이 상식 범위다')

const slow = new Promise((r) => setTimeout(() => r('늦게 옴'), 200))
try {
  await withTimeout(slow, 30)
  eq('끝남', '시간 초과여야 한다', '느린 일은 시간 초과로 끝난다')
} catch (e) {
  ok(isTimeout(e), '느린 일은 TimeoutError 로 끝난다')
  ok((e as Error).message.includes('끝나지 않았습니다'), '시간 초과 문구가 사람 말이다')
}

const fast = Promise.resolve('바로 옴')
eq(await withTimeout(fast, 1000), '바로 옴', '제때 끝나면 값이 그대로 온다')

try {
  await withTimeout(Promise.reject(new Error('읽지 못함')), 1000)
  eq('끝남', '오류여야 한다', '오류는 그대로 전해진다')
} catch (e) {
  ok(!isTimeout(e), '보통 오류는 시간 초과가 아니다')
  eq((e as Error).message, '읽지 못함', '오류 내용이 보존된다')
}

// 시간 초과 뒤 원래 일이 늦게 끝나도 이미 끝난 약속을 다시 건드리지 않는다
const late = new Promise((r) => setTimeout(() => r('한참 뒤'), 50))
let secondSettle = false
withTimeout(late, 10).catch(() => {
  if (secondSettle) fail++
  secondSettle = true
})
await new Promise((r) => setTimeout(r, 120))
ok(secondSettle, '시간 초과는 한 번만 일어난다')

ok(new TimeoutError(20000).message.includes('20초'), '제한 시간을 초 단위로 말한다')

console.log(fail === 0 ? '\ncheck-preview: 통과' : `\n${fail}건 실패`)
process.exit(fail === 0 ? 0 : 1)
