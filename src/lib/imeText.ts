/**
 * 한글 입력(IME 조합)을 깨뜨리지 않는 글자 입력 상태 기계.
 *
 * **왜 필요한가**
 * 한국어 IME 는 `ㅂ → 바 → 방` 처럼 **조합 중인 임시 글자**를 입력칸에 올려 둔다.
 * 이때 바깥에서 `value` 를 다시 써 넣으면 브라우저의 조합 버퍼와 칸의 값이 어긋나
 * 자모가 갈라지거나 겹쳐 들어간다.
 *
 * 실제로 났던 일: 추출 검토 화면의 품목 표시명 칸이
 * `value={서버값}` + `onChange → 서버 저장 → 목록 다시 읽기 → setState` 였다.
 * 글자를 한 번 칠 때마다 서버를 다녀와 **조합 도중에** 값을 덮어썼다.
 * 숫자 칸은 `defaultValue` + `onBlur` 라 멀쩡했던 이유도 같다(치는 동안 덮어쓰지 않는다).
 *
 * **규칙**
 *   1. 조합 중에는 바깥 값을 절대 받아들이지 않는다.
 *   2. 칸에 초점이 있고 사용자가 고친 상태면 바깥 값을 받지 않는다.
 *   3. 확정(commit)은 조합이 끝난 뒤 **초점을 잃거나 Enter** 를 눌렀을 때만 한다.
 *   3-1. Esc 는 저장하지 않고 마지막 바깥 값으로 되돌린다(조합 중에는 IME 의 것이므로 가로채지 않는다).
 *   4. 글자 자체는 어떤 가공도 하지 않는다. 사용자가 친 그대로다.
 *
 * 화면 없이 시험할 수 있도록 순수 함수로 두었다(`scripts/check-ime.ts`).
 */

export interface ImeState {
  /** 칸에 보이는 값 */
  draft: string
  /** 바깥(서버·부모)에서 마지막으로 받은 값 */
  external: string
  /** IME 조합이 진행 중인가 */
  composing: boolean
  /** 칸에 초점이 있는가 */
  focused: boolean
}

export type ImeEvent =
  | { type: 'focus' }
  | { type: 'blur' }
  | { type: 'compositionStart' }
  | { type: 'compositionEnd'; value: string }
  | { type: 'change'; value: string }
  /** 부모가 새 값을 내려보냈다 */
  | { type: 'external'; value: string }
  /** Enter 등으로 지금 확정하라 */
  | { type: 'commitRequest' }
  /** Esc — 고치던 것을 버리고 바깥 값으로 돌아간다 */
  | { type: 'cancel' }

export interface ImeResult {
  state: ImeState
  /** 바깥으로 내보낼 확정값. 없으면 null */
  commit: string | null
}

export function initialState(value: string): ImeState {
  return { draft: value, external: value, composing: false, focused: false }
}

export function reduce(state: ImeState, event: ImeEvent): ImeResult {
  switch (event.type) {
    case 'focus':
      return { state: { ...state, focused: true }, commit: null }

    case 'compositionStart':
      return { state: { ...state, composing: true }, commit: null }

    // 조합이 끝나면 완성된 글자가 들어온다
    case 'compositionEnd':
      return { state: { ...state, composing: false, draft: event.value }, commit: null }

    // 치는 동안에는 **오직 화면 값만** 바꾼다. 저장하지 않는다.
    case 'change':
      return { state: { ...state, draft: event.value }, commit: null }

    case 'blur': {
      const next = { ...state, focused: false, composing: false }
      if (state.draft !== state.external) {
        return { state: { ...next, external: state.draft }, commit: state.draft }
      }
      return { state: next, commit: null }
    }

    case 'commitRequest': {
      // 조합 중 Enter 는 한자 변환 등 IME 의 것이다. 가로채지 않는다.
      if (state.composing) return { state, commit: null }
      if (state.draft !== state.external) {
        return { state: { ...state, external: state.draft }, commit: state.draft }
      }
      return { state, commit: null }
    }

    // Esc — **저장하지 않고** 마지막 바깥 값으로 되돌린다.
    // 조합 중 Esc 는 IME 가 조합을 취소하는 데 쓰므로 가로채지 않는다.
    case 'cancel': {
      if (state.composing) return { state, commit: null }
      return { state: { ...state, draft: state.external }, commit: null }
    }

    case 'external': {
      // (1) 조합 중에는 절대 덮어쓰지 않는다 — 이 한 줄이 이번 버그의 핵심이다.
      if (state.composing) return { state, commit: null }
      // (2) 사용자가 고치는 중이면 덮어쓰지 않는다.
      if (state.focused && state.draft !== state.external) return { state, commit: null }
      return {
        state: { ...state, draft: event.value, external: event.value },
        commit: null,
      }
    }
  }
}

/** 여러 사건을 차례로 흘려 넣는다 (시험용) */
export function run(start: ImeState, events: ImeEvent[]): { state: ImeState; commits: string[] } {
  let state = start
  const commits: string[] = []
  for (const e of events) {
    const r = reduce(state, e)
    state = r.state
    if (r.commit !== null) commits.push(r.commit)
  }
  return { state, commits }
}

/**
 * 한글 한 글자를 치는 동안 브라우저가 만드는 사건 흐름.
 * 시험에서 실제 IME 를 흉내 내는 데 쓴다.
 *
 * 예: `steps('방', ['ㅂ', '바', '방'])`
 */
export function typeKorean(prefix: string, stages: string[]): ImeEvent[] {
  const out: ImeEvent[] = [{ type: 'compositionStart' }]
  for (const s of stages) {
    out.push({ type: 'change', value: prefix + s })
  }
  out.push({ type: 'compositionEnd', value: prefix + stages[stages.length - 1] })
  return out
}

/** 영문·숫자·기호는 조합이 없다. change 만 온다. */
export function typePlain(prefix: string, text: string): ImeEvent[] {
  const out: ImeEvent[] = []
  let cur = prefix
  for (const ch of text) {
    cur += ch
    out.push({ type: 'change', value: cur })
  }
  return out
}
