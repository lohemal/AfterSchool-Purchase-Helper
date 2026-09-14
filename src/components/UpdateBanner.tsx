import { useEffect, useState } from 'react'

/**
 * 새 버전 알림.
 *
 * ## 시작을 막지 않는다
 * 프로그램이 켜진 **뒤에** 조용히 한 번 확인한다. 인터넷이 없거나 확인에 실패해도
 * 아무 말 없이 넘어간다. 업데이트 확인 때문에 품의 작성이 막히면 안 된다.
 *
 * ## 사람이 눌러야 설치한다
 * 찾기만 하고 저절로 설치하지 않는다. 작업 도중에 프로그램이 갑자기 닫히면 안 된다.
 *
 * ## `relaunch()` 를 부르지 않는다
 * Windows 에서 `install()` 은 **돌아오지 않는다.** 설치 프로그램을 띄우고 앱을 스스로 끝내며,
 * 설치가 끝나면 NSIS 가 앱을 다시 켠다. 여기서 `relaunch()` 를 부르면 방금 뜬 설치 프로그램과
 * 경쟁해서 앱이 먼저 켜지는 바람에 파일을 바꾸지 못하고 **구 버전 그대로 남는다.**
 */
type Phase =
  | { kind: 'quiet' }
  | { kind: 'found'; version: string }
  | { kind: 'working'; message: string }
  | { kind: 'failed'; message: string }

// 화면이 다 뜬 뒤에 확인한다
const DELAY_MS = 3000

export default function UpdateBanner() {
  const [phase, setPhase] = useState<Phase>({ kind: 'quiet' })
  const [update, setUpdate] = useState<{ version: string; run: () => Promise<void> } | null>(null)

  useEffect(() => {
    let alive = true
    const timer = setTimeout(async () => {
      try {
        const { check } = await import('@tauri-apps/plugin-updater')
        const found = await check()
        if (!alive || !found) return // 없으면 조용히 넘어간다
        setUpdate({
          version: found.version,
          run: async () => {
            await found.downloadAndInstall()
          },
        })
        setPhase({ kind: 'found', version: found.version })
      } catch {
        // 확인에 실패해도 아무 말 하지 않는다 — 본 기능과 상관없는 일이다
      }
    }, DELAY_MS)
    return () => {
      alive = false
      clearTimeout(timer)
    }
  }, [])

  if (phase.kind === 'quiet' || update === null) return null

  async function install() {
    if (!update) return
    setPhase({ kind: 'working', message: '내려받아 설치하는 중입니다. 잠시 뒤 프로그램이 다시 켜집니다…' })
    try {
      await update.run()
      // 여기로 돌아오지 않는 것이 정상이다 (설치 프로그램이 앱을 끝낸다)
    } catch {
      setPhase({
        kind: 'failed',
        message: '업데이트를 하지 못했습니다. 지금 쓰시는 버전은 그대로 쓸 수 있습니다.',
      })
    }
  }

  return (
    <div className="update-banner">
      {phase.kind === 'found' && (
        <>
          <span>
            새 버전 <b>{phase.version}</b> 이 나왔습니다.
          </span>
          <button className="small primary" onClick={install}>
            지금 업데이트
          </button>
          <button className="small" onClick={() => setPhase({ kind: 'quiet' })}>
            나중에
          </button>
        </>
      )}
      {phase.kind === 'working' && <span>{phase.message}</span>}
      {phase.kind === 'failed' && (
        <>
          <span>{phase.message}</span>
          <button className="small" onClick={() => setPhase({ kind: 'quiet' })}>
            닫기
          </button>
        </>
      )}
    </div>
  )
}
