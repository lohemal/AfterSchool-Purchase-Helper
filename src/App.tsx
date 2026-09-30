import { useCallback, useEffect, useState } from 'react'
import * as ipc from './ipc'
import type { CheckRow, Work } from './ipc/types'
import UpdateBanner from './components/UpdateBanner'
import WorkPicker from './components/WorkPicker'
import SetupStep from './steps/SetupStep'
import QuotesStep from './steps/QuotesStep'
import ReviewStep from './steps/ReviewStep'
import SettlementStep from './steps/SettlementStep'
import AllocationStep from './steps/AllocationStep'
import PreviewStep from './steps/PreviewStep'
import GenerateStep from './steps/GenerateStep'

const STEPS = [
  { key: 'setup', label: '부서 설정', needsWork: false },
  { key: 'quotes', label: '견적서 등록', needsWork: true },
  { key: 'review', label: '추출 결과 확인', needsWork: true },
  { key: 'settlement', label: '정산자료 등록', needsWork: true },
  { key: 'allocation', label: '금액 배분·검증', needsWork: true },
  { key: 'preview', label: '품의자료 미리보기', needsWork: true },
  { key: 'generate', label: 'Excel 생성', needsWork: true },
] as const

type StepKey = (typeof STEPS)[number]['key']

export default function App() {
  const [step, setStep] = useState<StepKey>('setup')
  const [works, setWorks] = useState<Work[]>([])
  const [workId, setWorkId] = useState<number | null>(null)
  const [checks, setChecks] = useState<CheckRow[]>([])
  const [error, setError] = useState('')

  /**
   * 작업 목록을 다시 읽는다.
   *
   * `select` 를 주면 그 작업을 고른다(방금 만들었거나 이름을 바꾼 작업).
   * 주지 않거나 목록에 없으면 보고 있던 작업을 그대로 두고,
   * 그것마저 사라졌으면 맨 위 작업으로 간다.
   */
  const refreshWorks = useCallback(async (select?: number | null) => {
    try {
      const list = await ipc.listWorks()
      setWorks(list)
      setWorkId((cur) => {
        if (select != null && list.some((w) => w.id === select)) return select
        if (cur !== null && list.some((w) => w.id === cur)) return cur
        return list[0]?.id ?? null
      })
    } catch (e) {
      setError(ipc.errorMessage(e))
    }
  }, [])

  useEffect(() => {
    void refreshWorks()
  }, [refreshWorks])

  const refreshChecks = useCallback(async () => {
    if (workId === null) {
      setChecks([])
      return
    }
    try {
      setChecks(await ipc.runChecks(workId))
    } catch (e) {
      setError(ipc.errorMessage(e))
    }
  }, [workId])

  const errorCount = checks.filter((c) => c.status === 'error' && !c.acknowledged).length
  const warnCount = checks.filter((c) => c.status === 'warn').length

  /** 작업을 만들거나·이름을 바꾸거나·지운 뒤 목록을 다시 읽는다 */
  async function afterWorkChange(action: 'created' | 'renamed' | 'deleted', id: number | null) {
    await refreshWorks(id)
    // 새로 만들었으면 바로 견적서 등록으로 넘어간다.
    // 지웠을 때는 보던 단계에 그대로 둔다 — 남은 작업이 있으면 그 작업을 이어서 보고,
    // 하나도 없으면 「먼저 작업을 만들어 주세요」가 나온다.
    if (action === 'created' && id !== null) setStep('quotes')
  }

  const work = works.find((w) => w.id === workId) ?? null

  return (
    <div className="app">
      <nav className="sidebar">
        <h1>방과후 품의 도우미</h1>

        <WorkPicker
          works={works}
          workId={workId}
          onSelect={setWorkId}
          onChanged={afterWorkChange}
          onError={setError}
        />

        <ul className="steps">
          {STEPS.map((s, i) => {
            const disabled = s.needsWork && workId === null
            const isAlloc = s.key === 'allocation'
            return (
              <li key={s.key} className={step === s.key ? 'active' : ''}>
                <button disabled={disabled} onClick={() => setStep(s.key)}>
                  <span className="num">{i + 1}</span>
                  <span>{s.label}</span>
                  {isAlloc && errorCount > 0 && (
                    <span className="badge bad count">{errorCount}</span>
                  )}
                  {isAlloc && errorCount === 0 && warnCount > 0 && (
                    <span className="badge check count">{warnCount}</span>
                  )}
                </button>
              </li>
            )
          })}
        </ul>

        {/* 새 버전이 있을 때만 나타난다. 없으면 아무것도 그리지 않는다 */}
        <UpdateBanner />
      </nav>

      <main className="main">
        {error && (
          <div className="banner bad">
            {error}
            <button className="small" style={{ marginLeft: 10 }} onClick={() => setError('')}>
              닫기
            </button>
          </div>
        )}

        {step === 'setup' && <SetupStep onError={setError} />}
        {step === 'quotes' && workId !== null && (
          <QuotesStep workId={workId} onError={setError} onDone={() => setStep('review')} />
        )}
        {step === 'review' && workId !== null && <ReviewStep workId={workId} onError={setError} />}
        {step === 'settlement' && workId !== null && (
          <SettlementStep workId={workId} onError={setError} />
        )}
        {step === 'allocation' && workId !== null && (
          <AllocationStep workId={workId} onError={setError} onChecks={setChecks} />
        )}
        {step === 'preview' && workId !== null && <PreviewStep workId={workId} onError={setError} />}
        {step === 'generate' && workId !== null && work !== null && (
          <GenerateStep workId={workId} work={work} onError={setError} onRefresh={refreshChecks} />
        )}

        {STEPS.find((s) => s.key === step)?.needsWork && workId === null && (
          <div className="empty">먼저 작업을 만들어 주세요.</div>
        )}
      </main>
    </div>
  )
}
