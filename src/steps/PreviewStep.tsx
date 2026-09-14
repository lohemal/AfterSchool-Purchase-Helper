import { useCallback, useEffect, useState } from 'react'
import * as ipc from '../ipc'
import type { Fund, FundPreview } from '../ipc/types'
import { comma } from '../lib/money'

interface Props {
  workId: number
  onError: (m: string) => void
}

export default function PreviewStep({ workId, onError }: Props) {
  const [previews, setPreviews] = useState<FundPreview[]>([])
  const [tab, setTab] = useState<Fund>('beneficiary')

  const refresh = useCallback(async () => {
    try {
      setPreviews(await ipc.preview(workId))
    } catch (e) {
      onError(ipc.errorMessage(e))
    }
  }, [workId, onError])

  useEffect(() => {
    void refresh()
  }, [refresh])

  const cur = previews.find((p) => p.fund === tab)

  return (
    <>
      <header>
        <h2>6. 품의자료 미리보기</h2>
        <p>재원별로 실제 만들어질 줄을 그대로 보여 줍니다. 금액이 0인 줄은 만들지 않습니다.</p>
      </header>

      <div className="tabs">
        {previews.map((p) => (
          <button key={p.fund} className={p.fund === tab ? 'sel' : ''} onClick={() => setTab(p.fund)}>
            {p.fundLabel}
            {p.rows.length === 0 ? ' · 없음' : ` · ${p.rows.length}줄`}
          </button>
        ))}
      </div>

      {cur && (
        <div className="panel">
          <h3>
            {cur.fundLabel}
            <span className="sub">
              시트 이름 「품목내역」 · 열 내용 | 규격 | 수량 | 예상단가
            </span>
          </h3>

          {cur.rows.length === 0 ? (
            <div className="banner notice">{cur.emptyNote}</div>
          ) : (
            <>
              <div className="table-wrap">
                <table>
                  <thead>
                    <tr>
                      <th>내용</th>
                      <th style={{ width: 70 }}>규격</th>
                      <th style={{ width: 70 }} className="num">
                        수량
                      </th>
                      <th style={{ width: 130 }} className="num">
                        예상단가
                      </th>
                    </tr>
                  </thead>
                  <tbody>
                    {cur.rows.map((r, i) => (
                      <tr key={i}>
                        <td>{r.content}</td>
                        <td>식</td>
                        <td className="num">1</td>
                        <td className="num">{comma(r.amount)}</td>
                      </tr>
                    ))}
                    <tr className="settle-row">
                      <td colSpan={3}>합계</td>
                      <td className="num">{comma(cur.total)}</td>
                    </tr>
                  </tbody>
                </table>
              </div>
              <p className="note">
                합계 줄은 화면에서만 보여 줍니다. 만들어지는 파일에는 들어가지 않습니다.
              </p>
            </>
          )}
        </div>
      )}

      <div className="panel">
        <h3>재원별 요약</h3>
        <div className="table-wrap">
          <table>
            <thead>
              <tr>
                <th>재원</th>
                <th className="num">줄 수</th>
                <th className="num">합계</th>
                <th>파일</th>
              </tr>
            </thead>
            <tbody>
              {previews.map((p) => (
                <tr key={p.fund} className={p.rows.length === 0 ? 'dim' : ''}>
                  <td>{p.fundLabel}</td>
                  <td className="num">{p.rows.length}</td>
                  <td className="num">{comma(p.total)}</td>
                  <td>
                    {p.rows.length === 0 ? (
                      <span className="badge plain">대상 금액 없음 · 파일 생성 안 함</span>
                    ) : (
                      <span className="badge ok">생성</span>
                    )}
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      </div>
    </>
  )
}
