/**
 * 업데이트 확인의 상태와 순서 — 화면 없이 도는 순수한 부분.
 *
 * ## 왜 따로 떼어 두는가
 * 업데이트 확인은 **인터넷에 기대는 유일한 기능**이다. 실패가 흔하고,
 * 실패가 품의 작성을 막으면 안 된다. 그래서 「무엇을 보여 줄지」와
 * 「두 번 눌렀을 때 어떻게 되는지」를 화면과 떼어 놓고 시험한다
 * (`scripts/check-update.ts`).
 *
 * ## 여기서 하지 않는 일
 * 내려받기·서명 확인·설치는 **Tauri 업데이터가 한다.** 이 파일은 그 함수를
 * 언제 부르고 결과를 어떻게 말할지만 정한다. 서명 검증을 돌아가는 길은 없다.
 */

export type UpdateState =
  /** 아직 아무것도 하지 않았다 — 아무 말도 하지 않는다 */
  | { kind: 'idle' }
  | { kind: 'checking' }
  | { kind: 'latest' }
  | { kind: 'found'; version: string }
  | { kind: 'failed' }
  | { kind: 'installing' }
  | { kind: 'installFailed' }

/** 업데이터가 찾아낸 새 버전 */
export interface FoundUpdate {
  version: string
  /** 내려받아 설치한다. Windows 에서는 **돌아오지 않는 것이 정상**이다. */
  install: () => Promise<void>
}

/** 새 버전이 없으면 `null` */
export type CheckFn = () => Promise<FoundUpdate | null>

/** `0.1.1` 도 `v0.1.1` 도 화면에는 `v0.1.1` 로 적는다 */
export function withV(version: string): string {
  const v = version.trim()
  return v.startsWith('v') ? v : `v${v}`
}

/** 지금 상태를 사람이 읽는 한 줄로. `idle` 은 할 말이 없다. */
export function updateMessage(state: UpdateState): string {
  switch (state.kind) {
    case 'idle':
      return ''
    case 'checking':
      return '업데이트를 확인하고 있습니다.'
    case 'latest':
      return '현재 최신 버전입니다.'
    case 'found':
      return `새 버전 ${withV(state.version)}을 사용할 수 있습니다.`
    case 'failed':
      return '업데이트 정보를 확인하지 못했습니다.'
    case 'installing':
      return '내려받아 설치하는 중입니다. 잠시 뒤 프로그램이 다시 켜집니다.'
    case 'installFailed':
      return '업데이트를 하지 못했습니다. 지금 쓰시는 버전은 그대로 쓸 수 있습니다.'
  }
}

/** 나쁜 소식인가 (빨강으로 보일 줄) */
export function isBadNews(state: UpdateState): boolean {
  return state.kind === 'failed' || state.kind === 'installFailed'
}

/** 일이 진행 중이라 단추를 눌러도 소용없는 상태인가 */
export function isBusy(state: UpdateState): boolean {
  return state.kind === 'checking' || state.kind === 'installing'
}

export interface UpdateChecker {
  /** 새 버전을 찾아본다. 이미 돌고 있으면 **아무 일도 하지 않는다.** */
  check: () => Promise<void>
  /** 찾아 둔 새 버전을 설치한다. 찾은 것이 없거나 돌고 있으면 아무 일도 하지 않는다. */
  install: () => Promise<void>
  /** 지금 무언가 돌고 있는가 */
  busy: () => boolean
  /** 눌렀지만 이미 돌고 있어서 흘려보낸 횟수 (시험용) */
  droppedClicks: () => number
}

/**
 * 업데이트 확인기.
 *
 * ## 두 번 눌러도 두 번 돌지 않는다
 * 확인은 몇 초 걸린다. 그동안 단추를 다시 눌러도 요청을 **하나만** 보낸다.
 * 화면에서도 단추를 흐리게 하지만, 그것만으로는 모자라다 —
 * 자동 확인과 사용자의 확인이 겹칠 수도 있기 때문이다.
 *
 * ## 실패해도 던지지 않는다
 * `check()` 와 `install()` 은 어떤 경우에도 예외를 밖으로 내보내지 않는다.
 * 인터넷이 없어서 난 오류가 앱 전체 오류로 번지면 안 된다.
 */
export function createUpdateChecker(opts: {
  check: CheckFn
  onState: (state: UpdateState) => void
}): UpdateChecker {
  let busy = false
  let dropped = 0
  let found: FoundUpdate | null = null

  async function check() {
    if (busy) {
      dropped++
      return
    }
    busy = true
    opts.onState({ kind: 'checking' })
    try {
      const result = await opts.check()
      found = result
      opts.onState(result ? { kind: 'found', version: result.version } : { kind: 'latest' })
    } catch {
      // 까닭은 화면에 옮기지 않는다 — 사용자가 할 수 있는 일이 없고,
      // 자칫 경로 같은 것이 섞여 나올 수 있다 (로그 규칙).
      found = null
      opts.onState({ kind: 'failed' })
    } finally {
      busy = false
    }
  }

  async function install() {
    if (busy) {
      dropped++
      return
    }
    if (!found) return
    busy = true
    opts.onState({ kind: 'installing' })
    try {
      await found.install()
      // Windows 에서는 여기로 돌아오지 않는 것이 정상이다.
      // 설치 프로그램이 앱을 끝내고, 끝나면 다시 켜 준다.
    } catch {
      opts.onState({ kind: 'installFailed' })
    } finally {
      busy = false
    }
  }

  return {
    check,
    install,
    busy: () => busy,
    droppedClicks: () => dropped,
  }
}
