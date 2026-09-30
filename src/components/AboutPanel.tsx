import { useEffect, useMemo, useRef, useState } from 'react'
import { createUpdateChecker, isBadNews, isBusy, updateMessage, withV } from '../lib/updateCheck'
import type { UpdateState } from '../lib/updateCheck'
import { checkForUpdate, currentVersion } from '../lib/tauriUpdate'

/**
 * 프로그램 정보 — 지금 버전과 **직접 누르는 업데이트 확인**.
 *
 * 켤 때 조용히 하는 자동 확인(`UpdateBanner`)은 그대로 둔다.
 * 그쪽은 새 버전이 없으면 아무 말도 하지 않아서, 사용자가 「확인은 해 봤나」를 알 길이 없다.
 * 여기서는 눌렀을 때 **네 가지 결과를 모두 말로 보여 준다**.
 *
 * ## 설정 화면 맨 위에 둔다
 * 아래에 두었더니 부서가 스무 개쯤 되면 한참 굴려 내려야 닿았고,
 * 글이 화면 끝에 걸려 잘렸다. 맨 위는 스크롤 없이 늘 보인다.
 *
 * ## 확인에 실패해도 이 칸에만 한 줄이 뜬다
 * 앱 전체 오류로 올리지 않는다 — 인터넷이 없어도 품의 작성은 다 되어야 한다.
 */
export default function AboutPanel() {
  const [version, setVersion] = useState('')
  const [state, setState] = useState<UpdateState>({ kind: 'idle' })

  // 화면이 다시 그려져도 확인기는 하나만 쓴다 (그래야 중복 요청을 막을 수 있다)
  const stateRef = useRef(setState)
  stateRef.current = setState
  const checker = useMemo(
    () => createUpdateChecker({ check: checkForUpdate, onState: (s) => stateRef.current(s) }),
    [],
  )

  useEffect(() => {
    let alive = true
    void currentVersion()
      .then((v) => {
        if (alive) setVersion(v)
      })
      .catch(() => {
        // 버전을 못 읽어도 화면이 깨지면 안 된다
      })
    return () => {
      alive = false
    }
  }, [])

  const busy = isBusy(state)
  const message = updateMessage(state)

  return (
    <section className="about">
      <span className="about-label">프로그램 정보</span>
      <b className="about-version">{version ? withV(version) : '버전 확인 중…'}</b>

      <button className="small" disabled={busy} onClick={() => void checker.check()}>
        업데이트 확인
      </button>

      {state.kind === 'found' && (
        <button className="small primary" disabled={busy} onClick={() => void checker.install()}>
          업데이트 설치
        </button>
      )}

      {message && <span className={isBadNews(state) ? 'about-msg bad' : 'about-msg'}>{message}</span>}

      {state.kind === 'found' && (
        <span className="note">누르기 전에는 설치하지 않습니다.</span>
      )}
      {state.kind === 'failed' && (
        <span className="note">인터넷에 닿지 못했을 수 있습니다. 지금 버전은 그대로 쓸 수 있습니다.</span>
      )}
    </section>
  )
}
