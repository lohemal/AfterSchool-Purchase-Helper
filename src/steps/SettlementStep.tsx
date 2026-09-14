import { useCallback, useEffect, useState } from 'react'
import { open } from '@tauri-apps/plugin-dialog'
import * as ipc from '../ipc'
import type { Department, Funds, SettlementRowView, Warning } from '../ipc/types'
import { FUNDS, FUND_LABEL } from '../ipc/types'
import { comma } from '../lib/money'

interface Props {
  workId: number
  onError: (m: string) => void
}

export default function SettlementStep({ workId, onError }: Props) {
  const [rows, setRows] = useState<SettlementRowView[]>([])
  const [depts, setDepts] = useState<Department[]>([])
  const [warnings, setWarnings] = useState<Warning[]>([])

  const refresh = useCallback(async () => {
    try {
      setRows(await ipc.listSettlement(workId))
      setDepts(await ipc.listDepartments())
    } catch (e) {
      onError(ipc.errorMessage(e))
    }
  }, [workId, onError])

  useEffect(() => {
    void refresh()
  }, [refresh])

  async function pickFile() {
    try {
      const picked = await open({
        multiple: false,
        filters: [{ name: '정산자료', extensions: ['xlsx', 'xls', 'xlsm'] }],
      })
      if (!picked || Array.isArray(picked)) return
      const w = await ipc.loadSettlement(workId, picked)
      setWarnings(w)
      await refresh()
    } catch (e) {
      onError(ipc.errorMessage(e))
    }
  }

  async function attach(sourceName: string, deptId: string) {
    if (deptId === '') return
    try {
      await ipc.mapAlias(workId, sourceName, Number(deptId))
      await refresh()
    } catch (e) {
      onError(ipc.errorMessage(e))
    }
  }

  // 부서별 합산 결과 (별칭 여럿 → 한 부서)
  const grouped = new Map<string, { funds: Funds; sources: string[] }>()
  for (const r of rows) {
    if (r.departmentId === null) continue
    const key = r.departmentName
    const g = grouped.get(key) ?? {
      funds: { beneficiary: 0, excess: 0, subsidy: 0, voucher: 0 },
      sources: [],
    }
    g.funds = {
      beneficiary: g.funds.beneficiary + r.funds.beneficiary,
      excess: g.funds.excess + r.funds.excess,
      subsidy: g.funds.subsidy + r.funds.subsidy,
      voucher: g.funds.voucher + r.funds.voucher,
    }
    g.sources.push(r.sourceName)
    grouped.set(key, g)
  }

  const unmapped = rows.filter((r) => r.departmentId === null)

  return (
    <>
      <header>
        <h2>4. 정산자료 등록</h2>
        <p>
          정산자료의 「부서명」을 등록된 정산 별칭으로 찾아 품의 부서에 붙입니다. 같은 부서로 붙은
          줄의 금액은 자동으로 합산됩니다.
        </p>
      </header>

      <div className="panel">
        <div className="row">
          <button className="primary" onClick={pickFile}>
            정산자료 파일 고르기
          </button>
          <span className="note">
            읽은 뒤 행별 합과 열별 합을 검산합니다. 값은 고치지 않고 다를 때 알려만 줍니다.
          </span>
        </div>
        {warnings.length > 0 && (
          <ul className="warn-list">
            {warnings.map((w, i) => (
              <li key={i} className={w.severity}>
                {w.message}
              </li>
            ))}
          </ul>
        )}
      </div>

      {unmapped.length > 0 && (
        <div className="panel">
          <div className="banner bad">
            어느 부서에도 연결되지 않은 이름이 {unmapped.length}개 있습니다. 부서를 골라 주세요.
            고른 내용은 부서 설정에 저장되어 다음 달에도 그대로 쓰입니다.
          </div>
          <div className="table-wrap">
            <table>
              <thead>
                <tr>
                  <th>정산자료의 이름</th>
                  <th className="num">합계</th>
                  <th>붙일 품의 부서</th>
                </tr>
              </thead>
              <tbody>
                {unmapped.map((r) => (
                  <tr key={r.id}>
                    <td>{r.sourceName}</td>
                    <td className="num">{comma(r.funds.beneficiary + r.funds.excess + r.funds.subsidy + r.funds.voucher)}</td>
                    <td>
                      <select defaultValue="" onChange={(e) => attach(r.sourceName, e.target.value)}>
                        <option value="">— 고르기 —</option>
                        {depts.map((d) => (
                          <option key={d.id} value={d.id}>
                            {d.displayName}
                          </option>
                        ))}
                      </select>
                    </td>
                  </tr>
                ))}
              </tbody>
            </table>
          </div>
        </div>
      )}

      {rows.length > 0 && (
        <>
          <div className="panel">
            <h3>
              부서별 합산 결과<span className="sub">품의에 쓰이는 금액입니다</span>
            </h3>
            <div className="table-wrap">
              <table>
                <thead>
                  <tr>
                    <th>품의 부서</th>
                    <th>합쳐진 정산 별칭</th>
                    {FUNDS.map((f) => (
                      <th key={f} className="num">
                        {FUND_LABEL[f]}
                      </th>
                    ))}
                    <th className="num">합계</th>
                  </tr>
                </thead>
                <tbody>
                  {[...grouped.entries()].map(([name, g]) => (
                    <tr key={name}>
                      <td>{name}</td>
                      <td>
                        {g.sources.length > 1 ? (
                          <b>{g.sources.join(' + ')}</b>
                        ) : (
                          <span className="note">{g.sources[0]}</span>
                        )}
                      </td>
                      {FUNDS.map((f) => (
                        <td key={f} className="num">
                          {comma(g.funds[f])}
                        </td>
                      ))}
                      <td className="num">
                        <b>
                          {comma(
                            g.funds.beneficiary + g.funds.excess + g.funds.subsidy + g.funds.voucher,
                          )}
                        </b>
                      </td>
                    </tr>
                  ))}
                </tbody>
              </table>
            </div>
          </div>

          <div className="panel">
            <h3>
              읽어 온 원본<span className="sub">{rows.length}줄 — 프로그램이 고치지 않습니다</span>
            </h3>
            <div className="table-wrap">
              <table>
                <thead>
                  <tr>
                    <th className="num">줄</th>
                    <th>부서명(원문)</th>
                    <th>연결된 품의 부서</th>
                    {FUNDS.map((f) => (
                      <th key={f} className="num">
                        {FUND_LABEL[f]}
                      </th>
                    ))}
                    <th className="num">합계</th>
                  </tr>
                </thead>
                <tbody>
                  {rows.map((r) => (
                    <tr key={r.id} className={r.departmentId === null ? 'dim' : ''}>
                      <td className="num">{r.rowNo}</td>
                      <td>{r.sourceName}</td>
                      <td>
                        {r.departmentName || <span className="badge bad">미등록 별칭</span>}
                      </td>
                      {FUNDS.map((f) => (
                        <td key={f} className="num">
                          {comma(r.funds[f])}
                        </td>
                      ))}
                      <td className="num">{comma(r.statedTotal ?? 0)}</td>
                    </tr>
                  ))}
                </tbody>
              </table>
            </div>
          </div>
        </>
      )}

      {rows.length === 0 && <div className="empty">아직 정산자료를 읽지 않았습니다.</div>}
    </>
  )
}
