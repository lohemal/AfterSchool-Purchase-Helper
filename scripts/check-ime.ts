// 한글 입력(IME 조합) 규칙 시험.
// 실제 IME 를 띄울 수는 없으므로 브라우저가 만드는 **사건 흐름**을 그대로 흉내 낸다.
// 조합 중에 바깥 값이 끼어들어도 글자가 깨지지 않아야 한다.

import assert from 'node:assert/strict'
import { initialState, reduce, run, typeKorean, typePlain } from '../src/lib/imeText.ts'
import type { ImeEvent } from '../src/lib/imeText.ts'

// ---------------------------------------------------------------- 도우미

/** 한글 낱글자 조합 단계 (실제 IME 가 내보내는 중간 모습) */
const 방 = ['ㅂ', '바', '방']
const 과 = ['ㄱ', '과'.normalize(), '과']
const 후 = ['ㅎ', '후']

function focusThen(...events: ImeEvent[]): ImeEvent[] {
  return [{ type: 'focus' }, ...events]
}

// ---------------------------------------------------------------- 1. 기본

{
  // 치는 동안에는 **한 번도 확정되지 않는다**
  const { state, commits } = run(
    initialState(''),
    focusThen(...typeKorean('', 방), ...typeKorean('방', 과)),
  )
  assert.equal(state.draft, '방과')
  assert.deepEqual(commits, [], '치는 중에는 저장하지 않는다')

  // 초점을 잃을 때 한 번만 확정
  const after = run(state, [{ type: 'blur' }])
  assert.deepEqual(after.commits, ['방과'])
  assert.equal(after.state.external, '방과')
}

// ---------------------------------------------------------------- 2. 이번 버그 그 자체

{
  // 조합 도중에 서버가 옛 값을 되돌려 보낸다 (버그 때의 실제 상황)
  const events: ImeEvent[] = focusThen(
    { type: 'compositionStart' },
    { type: 'change', value: 'ㅂ' },
    // ↓ 저장 → 목록 다시 읽기 → setState 가 여기서 끼어들었다
    { type: 'external', value: '' },
    { type: 'change', value: '바' },
    { type: 'external', value: 'ㅂ' },
    { type: 'change', value: '방' },
    { type: 'compositionEnd', value: '방' },
  )
  const { state, commits } = run(initialState(''), events)
  assert.equal(state.draft, '방', '조합 중 바깥 값이 끼어들어도 글자가 깨지면 안 된다')
  assert.deepEqual(commits, [])
}

// ---------------------------------------------------------------- 3. 사용자가 말한 문자열들

const CASES: [string, ImeEvent[]][] = [
  [
    '방과후 기초Yap! 상',
    [
      ...typeKorean('', 방),
      ...typeKorean('방', 과),
      ...typeKorean('방과', 후),
      ...typePlain('방과후', ' '),
      ...typeKorean('방과후 ', ['ㄱ', '기']),
      ...typeKorean('방과후 기', ['ㅊ', '초']),
      ...typePlain('방과후 기초', 'Yap! '),
      ...typeKorean('방과후 기초Yap! ', ['ㅅ', '사', '상']),
    ],
  ],
  [
    '드론항공과학 교구세트',
    [
      ...typeKorean('', ['ㄷ', '드']),
      ...typeKorean('드', ['ㄹ', '로', '론']),
      ...typeKorean('드론', ['ㅎ', '하', '항']),
      ...typeKorean('드론항', ['ㄱ', '고', '공']),
      ...typeKorean('드론항공', ['ㄱ', '과']),
      ...typeKorean('드론항공과', ['ㅎ', '하', '학']),
      ...typePlain('드론항공과학', ' '),
      ...typeKorean('드론항공과학 ', ['ㄱ', '교']),
      ...typeKorean('드론항공과학 교', ['ㄱ', '구']),
      ...typeKorean('드론항공과학 교구', ['ㅅ', '세']),
      ...typeKorean('드론항공과학 교구세', ['ㅌ', '트']),
    ],
  ],
  [
    '바둑교재(상상바둑)',
    [
      ...typeKorean('', ['ㅂ', '바']),
      ...typeKorean('바', ['ㄷ', '두', '둑']),
      ...typeKorean('바둑', ['ㄱ', '교']),
      ...typeKorean('바둑교', ['ㅈ', '재']),
      ...typePlain('바둑교재', '('),
      ...typeKorean('바둑교재(', ['ㅅ', '사', '상']),
      ...typeKorean('바둑교재(상', ['ㅅ', '사', '상']),
      ...typeKorean('바둑교재(상상', ['ㅂ', '바']),
      ...typeKorean('바둑교재(상상바', ['ㄷ', '두', '둑']),
      ...typePlain('바둑교재(상상바둑', ')'),
    ],
  ],
  [
    '토탈공예미니어처',
    [
      ...typeKorean('', ['ㅌ', '토']),
      ...typeKorean('토', ['ㅌ', '타', '탈']),
      ...typeKorean('토탈', ['ㄱ', '고', '공']),
      ...typeKorean('토탈공', ['ㅇ', '예']),
      ...typeKorean('토탈공예', ['ㅁ', '미']),
      ...typeKorean('토탈공예미', ['ㄴ', '니']),
      ...typeKorean('토탈공예미니', ['ㅇ', '어']),
      ...typeKorean('토탈공예미니어', ['ㅊ', '처']),
    ],
  ],
]

for (const [expected, events] of CASES) {
  // (가) 방해 없이
  {
    const { state, commits } = run(initialState(''), focusThen(...events))
    assert.equal(state.draft, expected, `조용히 칠 때: ${expected}`)
    assert.deepEqual(commits, [], '치는 중 저장 없음')
    const done = run(state, [{ type: 'blur' }])
    assert.deepEqual(done.commits, [expected], `확정: ${expected}`)
  }

  // (나) **글자마다 바깥 값이 끼어드는** 최악의 경우 (버그 때의 상황)
  {
    const noisy: ImeEvent[] = []
    let seen = 0
    for (const e of events) {
      noisy.push(e)
      if (e.type === 'change') {
        seen += 1
        // 서버가 한 박자 늦은 값을 계속 밀어 넣는다
        noisy.push({ type: 'external', value: `지연된값${seen}` })
      }
    }
    const { state } = run(initialState(''), focusThen(...noisy))
    assert.equal(state.draft, expected, `방해받아도 같아야 한다: ${expected}`)
  }
}

// ---------------------------------------------------------------- 4. 지우기·섞어 치기

{
  // 한글 입력 중 Backspace — 조합이 되돌아간다
  const { state } = run(
    initialState(''),
    focusThen(
      { type: 'compositionStart' },
      { type: 'change', value: 'ㅂ' },
      { type: 'change', value: '바' },
      { type: 'change', value: '방' },
      { type: 'change', value: '바' }, // Backspace
      { type: 'change', value: 'ㅂ' }, // Backspace
      { type: 'compositionEnd', value: 'ㅂ' },
    ),
  )
  assert.equal(state.draft, 'ㅂ')
}

{
  // 확정된 글자를 Backspace 로 지우기 (조합 밖)
  const start = run(initialState(''), focusThen(...typeKorean('', 방), { type: 'blur' }))
  assert.deepEqual(start.commits, ['방'])
  const del = run(start.state, [{ type: 'focus' }, { type: 'change', value: '' }, { type: 'blur' }])
  assert.deepEqual(del.commits, [''], '지운 것도 확정된다')
}

{
  // 한글 + 숫자 + 기호 섞어 치기
  const { state } = run(
    initialState(''),
    focusThen(
      ...typeKorean('', ['ㄱ', '교']),
      ...typeKorean('교', ['ㅈ', '재']),
      ...typePlain('교재', 'A-1 (2026)'),
      ...typeKorean('교재A-1 (2026)', ['ㅂ', '보', '본']),
    ),
  )
  assert.equal(state.draft, '교재A-1 (2026)본')
}

{
  // 한글 → 영문 전환 (조합이 끝난 뒤 평범한 입력)
  const { state } = run(
    initialState(''),
    focusThen(...typeKorean('', ['ㅅ', '세', '셋']), ...typePlain('셋', 'set')),
  )
  assert.equal(state.draft, '셋set')
}

// ---------------------------------------------------------------- 5. 초점 이동과 값 동기화

{
  // 다른 칸으로 옮기면 확정되고, 그 뒤 서버 값이 내려오면 받아들인다
  const typed = run(initialState('옛이름'), focusThen({ type: 'change', value: '새이름' }))
  assert.deepEqual(typed.commits, [])
  const moved = run(typed.state, [{ type: 'blur' }])
  assert.deepEqual(moved.commits, ['새이름'])
  const synced = run(moved.state, [{ type: 'external', value: '새이름' }])
  assert.equal(synced.state.draft, '새이름')
}

{
  // 초점이 없을 때 서버 값이 바뀌면 그대로 따라간다 (다른 줄을 고른 경우)
  const s = run(initialState('가'), [{ type: 'external', value: '나' }])
  assert.equal(s.state.draft, '나')
}

{
  // 초점이 있고 사용자가 고치는 중이면 서버 값이 와도 덮지 않는다
  const s = run(
    initialState('가'),
    focusThen({ type: 'change', value: '가나' }, { type: 'external', value: '다' }),
  )
  assert.equal(s.state.draft, '가나')
}

{
  // 고친 것이 없으면 초점이 있어도 따라간다 (되돌리기 단추 등)
  const s = run(initialState('가'), focusThen({ type: 'external', value: '나' }))
  assert.equal(s.state.draft, '나')
}

// ---------------------------------------------------------------- 6. Enter 와 중복 확정

{
  const s1 = run(initialState(''), focusThen(...typeKorean('', 방), { type: 'commitRequest' }))
  assert.deepEqual(s1.commits, ['방'], 'Enter 로 확정')
  // 곧바로 초점을 잃어도 **다시 확정하지 않는다**
  const s2 = run(s1.state, [{ type: 'blur' }])
  assert.deepEqual(s2.commits, [], '같은 값을 두 번 저장하지 않는다')
}

{
  // 조합 중 Enter 는 IME 의 것이다 (한자 변환 등). 가로채지 않는다.
  const s = run(
    initialState(''),
    focusThen({ type: 'compositionStart' }, { type: 'change', value: '한' }, {
      type: 'commitRequest',
    }),
  )
  assert.deepEqual(s.commits, [])
  assert.equal(s.state.draft, '한')
}

{
  // 바뀐 것이 없으면 초점을 잃어도 저장하지 않는다
  const s = run(initialState('그대로'), [{ type: 'focus' }, { type: 'blur' }])
  assert.deepEqual(s.commits, [])
}

// ---------------------------------------------------------------- 7. 글자를 가공하지 않는다

{
  // 앞뒤 공백도 사용자가 친 그대로 (프로그램이 다듬지 않는다 — 설계안 14장 12번)
  const s = run(initialState(''), focusThen({ type: 'change', value: '  띄어쓰기 ' }, { type: 'blur' }))
  assert.deepEqual(s.commits, ['  띄어쓰기 '])
}

// ---------------------------------------------------------------- 8. 빠르게 연속 입력

{
  // 조합 사건이 겹쳐 들어와도(빠른 타자) 마지막 값이 남는다
  const fast: ImeEvent[] = []
  for (const ch of ['ㅌ', '토', '토ㅌ', '토타', '토탈']) {
    fast.push({ type: 'compositionStart' }, { type: 'change', value: ch })
  }
  fast.push({ type: 'compositionEnd', value: '토탈' })
  const s = run(initialState(''), focusThen(...fast))
  assert.equal(s.state.draft, '토탈')
  assert.equal(s.state.composing, false)
}

console.log('check-ime: 통과')

// ------------------------------------------------------------------ Esc (P4-4)

{
  // Esc 는 **저장하지 않고** 고치기 전 값으로 되돌린다
  const start = initialState('방과후 기초Yap! 상')
  const { state, commits } = run(start, [
    { type: 'focus' },
    ...typeKorean('', ['ㅅ', '사', '삭', '삭제']),
    { type: 'cancel' },
  ])
  assert.deepEqual(commits, [], 'Esc 는 아무것도 저장하지 않는다')
  assert.equal(state.draft, '방과후 기초Yap! 상', 'Esc 를 누르면 원래 값으로 돌아간다')
  assert.equal(state.external, '방과후 기초Yap! 상', '바깥 값은 건드리지 않는다')
}

{
  // 조합 중 Esc 는 IME 의 것이다 — 가로채면 한글이 깨진다
  const start = initialState('가')
  const mid = run(start, [{ type: 'focus' }, { type: 'compositionStart' }, { type: 'change', value: '가ㄴ' }])
  const after = run(mid.state, [{ type: 'cancel' }])
  assert.equal(after.state.draft, '가ㄴ', '조합 중 Esc 는 상태 기계가 손대지 않는다')
  assert.equal(after.state.composing, true)
}

{
  // Esc 로 되돌린 뒤 초점을 잃어도 저장이 일어나지 않는다
  const start = initialState('교재')
  const { commits } = run(start, [
    { type: 'focus' },
    ...typePlain('교재', 'X'),
    { type: 'cancel' },
    { type: 'blur' },
  ])
  assert.deepEqual(commits, [], 'Esc 뒤 blur 는 저장하지 않는다')
}

console.log('check-ime(Esc): 통과')
