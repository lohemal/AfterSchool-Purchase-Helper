import { useCallback, useEffect, useState } from 'react'
import * as ipc from '../ipc'
import TextField from '../components/TextField'
import type { CheckRow, DeptAllocationView, Fund, Funds } from '../ipc/types'
import { FUNDS, FUND_LABEL } from '../ipc/types'
import { fundsTotal, horizontalChecks, verticalChecks, withFund } from '../lib/allocation'
import { comma, diffText, parseMoney } from '../lib/money'

interface Props {
  workId: number
  onError: (m: string) => void
  onChecks: (c: CheckRow[]) => void
}

export default function AllocationStep({ workId, onError, onChecks }: Props) {
  const [depts, setDepts] = useState<DeptAllocationView[]>([])
  const [edited, setEdited] = useState<Record<number, Funds>>({})
  const [checks, setChecks] = useState<CheckRow[]>([])
  const [reason, setReason] = useState<Record<number, string>>({})

  const refresh = useCallback(async () => {
    try {
      const list = await ipc.listAllocations(workId)
      setDepts(list)
      setEdited({})
      const c = await ipc.runChecks(workId)
      setChecks(c)
      onChecks(c)
    } catch (e) {
      onError(ipc.errorMessage(e))
    }
  }, [workId, onError, onChecks])

  useEffect(() => {
    void refresh()
  }, [refresh])

  function current(vendorId: number, fallback: Funds): Funds {
    return edited[vendorId] ?? fallback
  }

  function change(vendorId: number, fallback: Funds, fund: Fund, text: string) {
    const v = parseMoney(text) ?? 0
    setEdited((e) => ({ ...e, [vendorId]: withFund(current(vendorId, fallback), fund, v) }))
  }

  async function saveVendor(vendorId: number, fallback: Funds) {
    const funds = current(vendorId, fallback)
    try {
      await ipc.saveAllocation(workId, vendorId, funds)
      await refresh()
    } catch (e) {
      onError(ipc.errorMessage(e))
    }
  }

  async function ack(checkId: number) {
    const text = reason[checkId] ?? ''
    try {
      await ipc.acknowledgeCheck(checkId, text)
      await refresh()
    } catch (e) {
      onError(ipc.errorMessage(e))
    }
  }

  const errors = checks.filter((c) => c.status === 'error')
  const openErrors = errors.filter((c) => !c.acknowledged)

  return (
    <>
      <header>
        <h2>5. 금액 배분·검증</h2>
        <p>
          거래처가 하나인 부서는 정산 금액이 그대로 들어갑니다. 여럿이면 재원 네 칸을 직접 넣어
          주세요. 프로그램은 금액을 나누거나 추정하지 않습니다.
        </p>
      </header>

      {openErrors.length === 0 ? (
        <div className="banner ok">모든 검증을 통과했습니다.</div>
      ) : (
        <div className="banner bad">
          맞지 않는 항목이 {openErrors.length}건 있습니다. 값을 고치거나, 업무상 의도된 차이라면
          아래에서 사유를 적고 확인해 주세요.
        </div>
      )}

      {depts.map((d) => {
        const vs = verticalChecks(d, edited)
        const hs = horizontalChecks(d, edited)
        const skip = !d.hasSettlement && d.vendors.every((v) => v.quoteTotal === null)
        if (skip) return null
        const single = d.vendors.length === 1 && d.hasSettlement

        return (
          <div className="alloc-dept" key={d.departmentId}>
            <header>
              <h4>{d.departmentName}</h4>
              {single && <span className="badge plain">거래처 하나 — 자동</span>}
              {!d.hasSettlement && <span className="badge bad">정산자료에 금액 없음</span>}
              <div className="spacer" />
              {vs.every((c) => c.ok) && hs.every((c) => c.ok) ? (
                <span className="badge ok">정상</span>
              ) : (
                <span className="badge bad">금액 불일치</span>
              )}
            </header>

            <div className="table-wrap">
              <table>
                <thead>
                  <tr>
                    <th>거래처 관리명</th>
                    {FUNDS.map((f) => (
                      <th key={f} className="num">
                        {FUND_LABEL[f]}
                      </th>
                    ))}
                    <th className="num">합계</th>
                    <th className="num">견적 총액</th>
                    <th>가로 검증</th>
                  </tr>
                </thead>
                <tbody>
                  {d.vendors.map((v) => {
                    const f = current(v.vendorUnitId, v.allocated)
                    const h = hs.find((x) => x.vendorUnitId === v.vendorUnitId)!
                    const dirty = edited[v.vendorUnitId] !== undefined
                    return (
                      <tr key={v.vendorUnitId}>
                        <td>
                          {v.mgmtName}
                          {v.missing && !dirty && <span className="raw">배분 미입력</span>}
                        </td>
                        {FUNDS.map((fund) => (
                          <td key={fund} className="num">
                            {/*
                              공통 TextField 를 쓴다. 치는 동안 값을 다시 써 넣으면
                              쉼표가 끼어들며 **글자 자리(캐럿)가 끝으로 튀어** 가운데 숫자를
                              고칠 수 없다. 여기서는 화면 값만 두고, 합계는 onDraftChange 로 따라간다.
                            */}
                            <TextField
                              numeric
                              disabled={v.locked}
                              ariaLabel={`${v.mgmtName} ${FUND_LABEL[fund]}`}
                              value={comma(f[fund])}
                              onDraftChange={(text) =>
                                change(v.vendorUnitId, v.allocated, fund, text)
                              }
                              onCommit={(text) =>
                                change(v.vendorUnitId, v.allocated, fund, text)
                              }
                            />
                          </td>
                        ))}
                        <td className="num">
                          <b>{comma(fundsTotal(f))}</b>
                        </td>
                        <td className="num">
                          {v.quoteTotal === null ? (
                            <span className="badge bad">견적 없음</span>
                          ) : (
                            comma(v.quoteTotal)
                          )}
                        </td>
                        <td>
                          {h.expected === null ? (
                            <span className="note">—</span>
                          ) : h.ok ? (
                            <span className="badge ok">일치</span>
                          ) : (
                            <span className="badge bad">{diffText(h.diff)}</span>
                          )}
                          {dirty && (
                            <button
                              className="small primary"
                              style={{ marginLeft: 6 }}
                              onClick={() => saveVendor(v.vendorUnitId, v.allocated)}
                            >
                              저장
                            </button>
                          )}
                        </td>
                      </tr>
                    )
                  })}

                  <tr className="settle-row">
                    <td>정산자료 합계</td>
                    {FUNDS.map((f) => (
                      <td key={f} className="num">
                        {comma(d.settlement[f])}
                      </td>
                    ))}
                    <td className="num">{comma(fundsTotal(d.settlement))}</td>
                    <td colSpan={2}></td>
                  </tr>

                  <tr className="verdict-row">
                    <td>세로 검증</td>
                    {FUNDS.map((f) => {
                      const c = vs.find((x) => x.fund === f)!
                      return (
                        <td key={f} className="num">
                          {c.ok ? (
                            <span className="badge ok">일치</span>
                          ) : (
                            <span className="badge bad">{diffText(c.diff)}</span>
                          )}
                        </td>
                      )
                    })}
                    <td colSpan={3}></td>
                  </tr>
                </tbody>
              </table>
            </div>
          </div>
        )
      })}

      {errors.length > 0 && (
        <div className="panel">
          <h3>
            확인이 필요한 항목<span className="sub">사유를 적어야 파일을 만들 수 있습니다</span>
          </h3>
          <div className="table-wrap">
            <table>
              <thead>
                <tr>
                  <th>내용</th>
                  <th className="num">차이</th>
                  <th style={{ width: 320 }}>확인 사유</th>
                  <th style={{ width: 90 }}></th>
                </tr>
              </thead>
              <tbody>
                {errors.map((c) => (
                  <tr key={c.id} className={c.acknowledged ? 'dim' : ''}>
                    <td>{c.label}</td>
                    <td className="num">{c.diff === null ? '—' : diffText(c.diff)}</td>
                    <td>
                      {c.acknowledged ? (
                        <span>{c.ackReason}</span>
                      ) : (
                        /* 한글 사유를 치는 동안 바깥 값이 끼어들지 않게 한다 */
                        <TextField
                          key={`reason-${c.id}`}
                          style={{ width: '100%' }}
                          placeholder="왜 이대로 진행해도 되는지 적어 주세요 (필수)"
                          value={reason[c.id] ?? ''}
                          /* 단추를 바로 켜려면 치는 동안의 값도 알아야 한다 */
                          onDraftChange={(v) => setReason((r) => ({ ...r, [c.id]: v }))}
                          onCommit={(v) => setReason((r) => ({ ...r, [c.id]: v }))}
                        />
                      )}
                    </td>
                    <td>
                      {c.acknowledged ? (
                        <button
                          className="small"
                          onClick={async () => {
                            try {
                              await ipc.unacknowledgeCheck(c.id)
                              await refresh()
                            } catch (e) {
                              onError(ipc.errorMessage(e))
                            }
                          }}
                        >
                          되돌리기
                        </button>
                      ) : (
                        <button
                          className="small"
                          disabled={(reason[c.id] ?? '').trim() === ''}
                          onClick={() => ack(c.id)}
                        >
                          확인함
                        </button>
                      )}
                    </td>
                  </tr>
                ))}
              </tbody>
            </table>
          </div>
          <p className="note">
            확인 사유는 이 작업에만 남습니다. 만들어지는 Excel 파일에는 들어가지 않습니다.
          </p>
        </div>
      )}

      {checks.filter((c) => c.status === 'warn').length > 0 && (
        <div className="panel">
          <h3>참고 사항</h3>
          <ul className="warn-list">
            {checks
              .filter((c) => c.status === 'warn')
              .map((c) => (
                <li key={c.id} className="warn">
                  {c.label}
                </li>
              ))}
          </ul>
        </div>
      )}
    </>
  )
}
