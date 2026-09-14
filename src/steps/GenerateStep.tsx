import { useCallback, useEffect, useState } from 'react'
import { open } from '@tauri-apps/plugin-dialog'
import { revealItemInDir } from '@tauri-apps/plugin-opener'
import * as ipc from '../ipc'
import type { GateResult, GenerateResult, Work } from '../ipc/types'
import { comma } from '../lib/money'

interface Props {
  workId: number
  work: Work
  onError: (m: string) => void
  onRefresh: () => void
}

export default function GenerateStep({ workId, work, onError, onRefresh }: Props) {
  const [gate, setGate] = useState<GateResult | null>(null)
  const [dir, setDir] = useState<string>('')
  const [result, setResult] = useState<GenerateResult | null>(null)
  const [busy, setBusy] = useState(false)

  const refresh = useCallback(async () => {
    try {
      await ipc.runChecks(workId)
      setGate(await ipc.generationGate(workId))
      const last = await ipc.lastOutputDir()
      if (last) setDir((d) => d || last)
    } catch (e) {
      onError(ipc.errorMessage(e))
    }
  }, [workId, onError])

  useEffect(() => {
    void refresh()
  }, [refresh])

  async function pickDir() {
    try {
      const picked = await open({ directory: true, multiple: false, defaultPath: dir || undefined })
      if (picked && !Array.isArray(picked)) setDir(picked)
    } catch (e) {
      onError(ipc.errorMessage(e))
    }
  }

  async function run() {
    if (dir === '') return
    setBusy(true)
    try {
      const r = await ipc.generate(workId, dir)
      setResult(r)
      onRefresh()
    } catch (e) {
      onError(ipc.errorMessage(e))
    } finally {
      setBusy(false)
    }
  }

  const names = ['수익자', '초과금', '지원금', '자유수강권'].map((f) =>
    [work.schoolYear, work.month, work.kind, f].filter((s) => s.trim() !== '').join('_') + '.xlsx',
  )

  return (
    <>
      <header>
        <h2>7. Excel 생성</h2>
        <p>
          만든 뒤 바로 다시 읽어 시트 이름·머리글·값·자료형을 검사합니다. 검사에 실패하면 파일을
          남기지 않습니다.
        </p>
      </header>

      {gate && (
        <div className={`banner ${gate.canGenerate ? 'ok' : 'bad'}`}>
          {gate.message}
          {gate.openBlockers.length > 0 && (
            <ul className="warn-list">
              {gate.openBlockers.slice(0, 8).map((b) => (
                <li key={b.checkId} className="error">
                  {b.label}
                </li>
              ))}
              {gate.openBlockers.length > 8 && <li className="error">… 외 {gate.openBlockers.length - 8}건</li>}
            </ul>
          )}
          {gate.acknowledged.length > 0 && (
            <p className="note" style={{ marginTop: 6 }}>
              사유를 적고 확인한 항목 {gate.acknowledged.length}건은 작업 기록에 남습니다.
            </p>
          )}
        </div>
      )}

      <div className="panel">
        <h3>저장 위치</h3>
        <div className="row">
          <input
            type="text"
            style={{ flex: 1 }}
            value={dir}
            placeholder="폴더를 골라 주세요"
            onChange={(e) => setDir(e.target.value)}
          />
          <button onClick={pickDir}>폴더 고르기</button>
        </div>
        <p className="note">마지막으로 쓴 폴더를 기억합니다.</p>

        <h3 style={{ marginTop: 16 }}>만들어질 파일 이름</h3>
        <ul className="warn-list">
          {names.map((n) => (
            <li key={n} className="info">
              {n}
            </li>
          ))}
        </ul>
        <p className="note">대상 금액이 없는 재원은 만들지 않습니다.</p>

        <div className="row" style={{ marginTop: 16 }}>
          <button
            className="primary"
            disabled={busy || dir === '' || !(gate?.canGenerate ?? false)}
            onClick={run}
          >
            {busy ? '만드는 중…' : 'Excel 만들기'}
          </button>
        </div>
      </div>

      {result && (
        <div className="panel">
          <h3>결과</h3>
          {result.created.length > 0 && (
            <div className="table-wrap">
              <table>
                <thead>
                  <tr>
                    <th>재원</th>
                    <th className="num">줄 수</th>
                    <th className="num">합계</th>
                    <th>파일</th>
                    <th></th>
                  </tr>
                </thead>
                <tbody>
                  {result.created.map((f) => (
                    <tr key={f.path}>
                      <td>{f.fundLabel}</td>
                      <td className="num">{f.rowCount}</td>
                      <td className="num">{comma(f.total)}</td>
                      <td className="cellref">{f.path}</td>
                      <td>
                        <button
                          className="small"
                          onClick={() => revealItemInDir(f.path).catch((e) => onError(ipc.errorMessage(e)))}
                        >
                          폴더 열기
                        </button>
                      </td>
                    </tr>
                  ))}
                </tbody>
              </table>
            </div>
          )}
          {result.skipped.length > 0 && (
            <ul className="warn-list" style={{ marginTop: 10 }}>
              {result.skipped.map((s, i) => (
                <li key={i} className="info">
                  {s}
                </li>
              ))}
            </ul>
          )}
          <div className="banner ok" style={{ marginTop: 12 }}>
            만든 파일을 모두 다시 읽어 검사했습니다. 시트 이름은 「품목내역」, 열은 내용·규격·수량·예상단가입니다.
          </div>
        </div>
      )}
    </>
  )
}
