/**
 * 업데이트 확인 화면의 규칙 시험 (브라우저 없이 도는 순수 함수).
 *   npm run check:update
 *
 * 여기서 보는 것 넷:
 *   1. 네 가지 결과(확인 중·최신·새 버전·실패)를 모두 말로 보여 주는가
 *   2. 두 번 눌러도 요청이 한 번만 나가는가
 *   3. 확인이 실패해도 **예외를 밖으로 던지지 않는가** (앱 전체 오류가 되면 안 된다)
 *   4. 새 버전을 찾았어도 **누르기 전에는 설치하지 않는가**
 */
import {
  createUpdateChecker,
  isBadNews,
  isBusy,
  updateMessage,
  withV,
} from '../src/lib/updateCheck.ts'
import type { FoundUpdate, UpdateState } from '../src/lib/updateCheck.ts'

let fail = 0
function eq(got: unknown, want: unknown, what: string) {
  if (JSON.stringify(got) !== JSON.stringify(want)) {
    console.error(`✗ ${what}\n  받음: ${JSON.stringify(got)}\n  기대: ${JSON.stringify(want)}`)
    fail++
  } else {
    console.log(`✓ ${what}`)
  }
}

// ---------------------------------------------------------------- 보여 주는 말

eq(updateMessage({ kind: 'idle' }), '', '아직 안 눌렀으면 할 말이 없다')
eq(updateMessage({ kind: 'checking' }), '업데이트를 확인하고 있습니다.', '확인 중')
eq(updateMessage({ kind: 'latest' }), '현재 최신 버전입니다.', '최신')
eq(
  updateMessage({ kind: 'found', version: '0.1.2' }),
  '새 버전 v0.1.2을 사용할 수 있습니다.',
  '새 버전 있음',
)
eq(updateMessage({ kind: 'failed' }), '업데이트 정보를 확인하지 못했습니다.', '확인 실패')

eq(withV('0.1.1'), 'v0.1.1', '버전 앞에 v 를 붙인다')
eq(withV('v0.1.1'), 'v0.1.1', '이미 v 가 있으면 두 번 붙이지 않는다')
eq(withV(' 0.1.1 '), 'v0.1.1', '앞뒤 공백을 뗀다')

eq(isBadNews({ kind: 'failed' }), true, '확인 실패는 빨갛게')
eq(isBadNews({ kind: 'installFailed' }), true, '설치 실패도 빨갛게')
eq(isBadNews({ kind: 'latest' }), false, '최신은 빨갛지 않다')
eq(isBadNews({ kind: 'found', version: '0.1.2' }), false, '새 버전은 빨갛지 않다')

eq(isBusy({ kind: 'checking' }), true, '확인 중에는 단추를 잠근다')
eq(isBusy({ kind: 'installing' }), true, '설치 중에도 잠근다')
for (const s of ['idle', 'latest', 'failed', 'installFailed'] as const) {
  eq(isBusy({ kind: s }), false, `${s} 에서는 다시 누를 수 있다`)
}

// ---------------------------------------------------------------- 확인기

/** 원할 때 끝낼 수 있는 가짜 확인 함수 */
function pending<T>() {
  let settle!: (v: T) => void
  let reject!: (e: unknown) => void
  const promise = new Promise<T>((res, rej) => {
    settle = res
    reject = rej
  })
  return { promise, settle, reject }
}

function trace() {
  const states: UpdateState[] = []
  return { states, onState: (s: UpdateState) => states.push(s), kinds: () => states.map((s) => s.kind) }
}

// (1) 새 버전이 없으면 '최신'
{
  const t = trace()
  let calls = 0
  const c = createUpdateChecker({
    check: async () => {
      calls++
      return null
    },
    onState: t.onState,
  })
  await c.check()
  eq(t.kinds(), ['checking', 'latest'], '없으면 확인 중 → 최신')
  eq(calls, 1, '한 번만 물어본다')
  eq(c.busy(), false, '끝나면 다시 누를 수 있다')
}

// (2) 새 버전이 있으면 '찾음'. **설치는 저절로 되지 않는다.**
{
  const t = trace()
  let installed = 0
  const found: FoundUpdate = {
    version: '0.1.2',
    install: async () => {
      installed++
    },
  }
  const c = createUpdateChecker({ check: async () => found, onState: t.onState })
  await c.check()
  eq(t.states[1], { kind: 'found', version: '0.1.2' }, '버전까지 알려 준다')
  eq(installed, 0, '찾기만 하고 저절로 설치하지 않는다')

  await c.install()
  eq(installed, 1, '누르면 그때 설치한다')
  eq(t.states[2], { kind: 'installing' }, '설치 중이라고 알려 준다')
}

// (3) 확인이 실패해도 던지지 않는다
{
  const t = trace()
  const c = createUpdateChecker({
    check: async () => {
      throw new Error('getaddrinfo ENOTFOUND github.com')
    },
    onState: t.onState,
  })
  let threw = false
  try {
    await c.check()
  } catch {
    threw = true
  }
  eq(threw, false, '인터넷 오류를 밖으로 던지지 않는다')
  eq(t.kinds(), ['checking', 'failed'], '확인 중 → 실패')
  // 오류 내용(경로·주소)이 화면 문구에 섞여 나오면 안 된다
  eq(updateMessage(t.states[1]).includes('github'), false, '오류 속 주소를 화면에 옮기지 않는다')
}

// (4) 실패한 뒤 다시 누르면 다시 확인한다
{
  const t = trace()
  let calls = 0
  const c = createUpdateChecker({
    check: async () => {
      calls++
      if (calls === 1) throw new Error('일시적 오류')
      return null
    },
    onState: t.onState,
  })
  await c.check()
  await c.check()
  eq(calls, 2, '실패한 뒤에도 다시 확인할 수 있다')
  eq(t.kinds(), ['checking', 'failed', 'checking', 'latest'], '두 번째는 최신')
}

// (5) **중복 클릭·동시 요청 방지** — 돌고 있는 동안 눌러도 요청은 하나뿐
{
  const t = trace()
  let calls = 0
  const gate = pending<null>()
  const c = createUpdateChecker({
    check: async () => {
      calls++
      return await gate.promise
    },
    onState: t.onState,
  })
  const first = c.check()
  // 아직 안 끝난 사이에 네 번 더 누른다
  const rest = [c.check(), c.check(), c.check(), c.check()]
  await Promise.all(rest)
  eq(calls, 1, '다섯 번 눌러도 요청은 한 번')
  eq(c.droppedClicks(), 4, '나머지 네 번은 흘려보낸다')
  eq(c.busy(), true, '아직 확인 중')
  eq(t.kinds(), ['checking'], '상태도 한 번만 바뀐다')

  gate.settle(null)
  await first
  eq(t.kinds(), ['checking', 'latest'], '끝나면 결과를 알려 준다')
  eq(c.busy(), false, '끝나면 잠금이 풀린다')

  // 이제는 다시 누를 수 있다
  await c.check()
  eq(calls, 2, '끝난 뒤에는 다시 확인한다')
}

// (6) 확인 중에 설치를 눌러도 설치하지 않는다
{
  const gate = pending<null>()
  let installed = 0
  const c = createUpdateChecker({
    check: async () => await gate.promise,
    onState: () => {},
  })
  const first = c.check()
  await c.install()
  eq(installed, 0, '확인 중에 누른 설치는 무시한다')
  gate.settle(null)
  await first
}

// (7) 찾은 것이 없으면 설치를 눌러도 아무 일도 없다
{
  const t = trace()
  const c = createUpdateChecker({ check: async () => null, onState: t.onState })
  await c.check()
  await c.install()
  eq(t.kinds(), ['checking', 'latest'], '설치할 것이 없으면 상태가 바뀌지 않는다')
}

// (8) 설치가 실패해도 던지지 않고, 쓰던 버전은 그대로 쓸 수 있다고 알린다
{
  const t = trace()
  const c = createUpdateChecker({
    check: async () => ({
      version: '0.1.2',
      install: async () => {
        throw new Error('설치 프로그램을 띄우지 못했다')
      },
    }),
    onState: t.onState,
  })
  await c.check()
  let threw = false
  try {
    await c.install()
  } catch {
    threw = true
  }
  eq(threw, false, '설치 오류도 밖으로 던지지 않는다')
  eq(t.kinds(), ['checking', 'found', 'installing', 'installFailed'], '설치 실패까지 말해 준다')
  eq(
    updateMessage(t.states[3]),
    '업데이트를 하지 못했습니다. 지금 쓰시는 버전은 그대로 쓸 수 있습니다.',
    '실패해도 쓰던 버전은 그대로',
  )
}

console.log(fail === 0 ? '\ncheck-update: 통과' : `\ncheck-update: ${fail}건 실패`)
process.exit(fail === 0 ? 0 : 1)
