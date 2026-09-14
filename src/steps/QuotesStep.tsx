import { useCallback, useEffect, useState } from 'react'
import { open } from '@tauri-apps/plugin-dialog'
import * as ipc from '../ipc'
import type { Department, QuoteProgress, QuoteRow } from '../ipc/types'
import { comma } from '../lib/money'
import { SOURCE_LABEL, sourceNote, TRUST_BADGE, TRUST_LABEL } from '../lib/quoteTrust'

interface Props {
  workId: number
  onError: (m: string) => void
  onDone: () => void
}

const EXTS = ['xlsx', 'xls', 'xlsm', 'hwp', 'hwpx', 'pdf', 'jpg', 'jpeg', 'png']

export default function QuotesStep({ workId, onError, onDone }: Props) {
  const [quotes, setQuotes] = useState<QuoteRow[]>([])
  const [depts, setDepts] = useState<Department[]>([])
  const [problems, setProblems] = useState<string[]>([])
  const [hangul, setHangul] = useState<boolean | null>(null)
  const [ocr, setOcr] = useState<boolean | null>(null)
  const [busy, setBusy] = useState(false)
  const [progress, setProgress] = useState<QuoteProgress | null>(null)

  const refresh = useCallback(async () => {
    try {
      setQuotes(await ipc.listQuotes(workId))
      setDepts(await ipc.listDepartments())
    } catch (e) {
      onError(ipc.errorMessage(e))
    }
  }, [workId, onError])

  useEffect(() => {
    void refresh()
    void ipc.hangulAvailable().then(setHangul).catch(() => setHangul(null))
    void ipc.ocrAvailable().then(setOcr).catch(() => setOcr(null))
  }, [refresh])

  // 사진·PDF 는 한 장에 20초쯤 걸린다. 진행 상황을 받아 화면이 멈춘 것처럼 보이지 않게 한다.
  useEffect(() => {
    let stop: (() => void) | null = null
    let dead = false
    void ipc
      .onQuoteProgress((p) => setProgress(p))
      .then((un) => {
        if (dead) un()
        else stop = un
      })
    return () => {
      dead = true
      stop?.()
    }
  }, [])

  async function pickFiles() {
    try {
      const picked = await open({
        multiple: true,
        filters: [
          { name: '견적서', extensions: [...EXTS, 'pdf', 'jpg', 'jpeg', 'png'] },
          { name: '모든 파일', extensions: ['*'] },
        ],
      })
      if (!picked) return
      const paths = Array.isArray(picked) ? picked : [picked]
      setBusy(true)
      const probs = await ipc.registerQuotes(workId, paths)
      setProblems(probs)
      await refresh()
    } catch (e) {
      onError(ipc.errorMessage(e))
    } finally {
      setBusy(false)
      setProgress(null)
    }
  }

  async function changeVendor(quoteId: number, value: string) {
    try {
      await ipc.setQuoteVendor(quoteId, value === '' ? null : Number(value))
      await refresh()
    } catch (e) {
      onError(ipc.errorMessage(e))
    }
  }

  async function remove(q: QuoteRow) {
    if (!confirm(`'${q.sourceName}' 을 목록에서 지울까요?`)) return
    try {
      await ipc.deleteQuote(q.id)
      await refresh()
    } catch (e) {
      onError(ipc.errorMessage(e))
    }
  }

  const vendors = depts.flatMap((d) =>
    d.vendors.map((v) => ({ id: v.id, label: `${d.displayName} / ${v.mgmtName}` })),
  )

  function statusOf(q: QuoteRow) {
    if (q.vendorUnitId === null) return <span className="badge bad">거래처 매칭 실패</span>
    if (q.parseStatus === 'failed') return <span className="badge check">직접 입력 필요</span>
    // 사진·PDF 에서 읽었으면 근거로 판단한 신뢰 상태를 그대로 쓴다 (지어낸 확률이 아니다)
    if (q.source !== 'structured' && q.trust)
      return <span className={'badge ' + TRUST_BADGE[q.trust]}>{TRUST_LABEL[q.trust]}</span>
    if (q.warnings.some((w) => w.severity === 'warn'))
      return <span className="badge check">확인 필요</span>
    if (q.warnings.length > 0) return <span className="badge notice">주의</span>
    return <span className="badge ok">정상</span>
  }

  return (
    <>
      <header>
        <h2>2. 견적서 등록</h2>
        <p>
          파일명에 부서명(거래처가 여럿이면 관리명까지)이 들어 있으면 자동으로 거래처가 정해집니다.
        </p>
      </header>

      {ocr === false && (
        <div className="banner notice">
          이 컴퓨터에 한국어 글자 인식 기능이 없어 사진·스캔 PDF 는 읽지 못합니다. 설정 &gt; 시간 및
          언어 &gt; 언어에서 한국어의 &lsquo;광학 문자 인식&rsquo;을 더하거나, 품목을 직접 입력해
          주세요. 다른 형식은 그대로 읽힙니다.
        </div>
      )}

      {hangul === false && (
        <div className="banner notice">
          이 컴퓨터에서 한글(HWP) 프로그램을 찾지 못했습니다. HWP 견적서는 읽을 수 없고, 품목을 직접
          입력해야 합니다. XLSX·HWPX 는 그대로 읽힙니다.
        </div>
      )}

      <div className="panel">
        <div className="row">
          <button className="primary" onClick={pickFiles} disabled={busy}>
            {busy ? '읽는 중…' : '견적서 파일 고르기'}
          </button>
          <span className="note">
            읽을 수 있는 형식: XLSX · XLS · XLSM · HWPX · HWP · PDF · 사진(JPG · PNG). 사진과 스캔
            PDF 는 한 장에 20초쯤 걸리고, 읽은 뒤에는 꼭 사람이 한 번 확인해야 합니다.
          </span>
        </div>
        {busy && progress && (
          <div className="banner notice" style={{ marginTop: 12 }}>
            <b>
              ({progress.index}/{progress.total}) {progress.name}
            </b>
            <div className="raw">
              {progress.stage === 'failed' ? progress.message : progress.message || '읽는 중입니다.'}
            </div>
          </div>
        )}
        {problems.length > 0 && (
          <div className="banner bad" style={{ marginTop: 12 }}>
            <b>등록하지 못한 파일</b>
            <ul className="warn-list">
              {problems.map((p, i) => (
                <li key={i} className="error">
                  {p}
                </li>
              ))}
            </ul>
          </div>
        )}
      </div>

      <div className="panel">
        <h3>
          등록된 견적서<span className="sub">{quotes.length}건</span>
        </h3>
        {quotes.length === 0 ? (
          <div className="empty">아직 등록한 견적서가 없습니다.</div>
        ) : (
          <div className="table-wrap">
            <table>
              <thead>
                <tr>
                  <th>파일</th>
                  <th>형식</th>
                  <th>거래처 (부서 / 관리명)</th>
                  <th className="num">견적 총액</th>
                  <th>상태</th>
                  <th></th>
                </tr>
              </thead>
              <tbody>
                {quotes.map((q) => (
                  <tr key={q.id}>
                    <td>
                      {q.sourceName}
                      {!q.sourceExists && (
                        <span className="raw" style={{ color: 'var(--check)' }}>
                          원본 파일 없음 (읽어 둔 내용은 그대로 씁니다)
                        </span>
                      )}
                      {q.matchNote && <span className="raw">{q.matchNote}</span>}
                      {sourceNote(q) && (
                        <span className="raw" style={{ color: 'var(--check)' }}>
                          {sourceNote(q)}
                        </span>
                      )}
                      {q.parseError && (
                        <span className="raw" style={{ color: 'var(--check)' }}>
                          {q.parseError}
                        </span>
                      )}
                    </td>
                    <td>
                      {q.format}
                      {q.parseStatus === 'ok' && q.source !== 'structured' && (
                        <span className="raw">{SOURCE_LABEL[q.source]}</span>
                      )}
                    </td>
                    <td>
                      <select
                        value={q.vendorUnitId ?? ''}
                        onChange={(e) => changeVendor(q.id, e.target.value)}
                      >
                        <option value="">— 고르지 않음 —</option>
                        {vendors.map((v) => (
                          <option key={v.id} value={v.id}>
                            {v.label}
                          </option>
                        ))}
                      </select>
                      {q.matchMethod === 'auto' && <span className="raw">자동</span>}
                      {q.matchMethod === 'manual' && <span className="raw">직접 고름</span>}
                    </td>
                    <td className="num">{q.parseStatus === 'ok' ? comma(q.compareTotal) : '—'}</td>
                    <td>{statusOf(q)}</td>
                    <td>
                      <button className="small danger" onClick={() => remove(q)}>
                        지우기
                      </button>
                    </td>
                  </tr>
                ))}
              </tbody>
            </table>
          </div>
        )}
        {quotes.length > 0 && (
          <div className="row" style={{ marginTop: 12 }}>
            <div className="spacer" />
            <button className="primary" onClick={onDone}>
              추출 결과 확인으로
            </button>
          </div>
        )}
      </div>
    </>
  )
}
