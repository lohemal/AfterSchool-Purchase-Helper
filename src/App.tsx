import { useCallback, useEffect, useState } from 'react'
import * as ipc from './ipc'
import type { CheckRow, Work } from './ipc/types'
import UpdateBanner from './components/UpdateBanner'
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

  const refreshWorks = useCallback(async () => {
    try {
      const list = await ipc.listWorks()
      setWorks(list)
      setWorkId((cur) => (cur !== null && list.some((w) => w.id === cur) ? cur : list[0]?.id ?? null))
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

  async function newWork() {
    const now = new Date()
    const year = now.getMonth() + 1 >= 3 ? now.getFullYear() : now.getFullYear() - 1
    const schoolYear = `${year}학년도`
    const month = `${now.getMonth() + 1}월`
    const title = `${schoolYear} ${month} 교재비`
    try {
      const id = await ipc.createWork(title, schoolYear, month, '교재비')
      await refreshWorks()
      setWorkId(id)
      setStep('quotes')
    } catch (e) {
      setError(ipc.errorMessage(e))
    }
  }

  const work = works.find((w) => w.id === workId) ?? null

  return (
    <div className="app">
      <nav className="sidebar">
        <h1>방과후 품의 도우미</h1>

        <div className="work-pick">
          <label className="field">
            작업
            <select
              value={workId ?? ''}
              onChange={(e) => setWorkId(e.target.value === '' ? null : Number(e.target.value))}
            >
              {works.length === 0 && <option value="">작업 없음</option>}
              {works.map((w) => (
                <option key={w.id} value={w.id}>
                  {w.title}
                </option>
              ))}
            </select>
          </label>
          <button className="small" style={{ marginTop: 6, width: '100%' }} onClick={newWork}>
            새 작업 만들기
          </button>
        </div>

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
