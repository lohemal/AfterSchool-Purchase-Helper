/**
 * P4-4 확인용 화면. **제품 코드가 아니다.**
 *
 * 순수 함수 시험(`scripts/check-review.ts`)이 못 보는 것만 여기서 눈으로 본다.
 *   - Enter 로 같은 칸의 다음 줄로 내려가는가
 *   - Shift+Enter 로 윗줄로 올라가는가
 *   - Esc 로 고치기 전 값으로 돌아가는가 (저장하지 않는다)
 *   - 새 줄에 초점이 자동으로 가는가
 *
 * 띄우기: npm run dev → http://localhost:1420/test/ui/keys.html
 */
import { useRef, useState } from 'react'
import { createRoot } from 'react-dom/client'
import TextField from '../../src/components/TextField'
import { nextFieldIndex } from '../../src/lib/gridKeys'

interface Row {
  id: number
  name: string
  spec: string
  qty: string
  price: string
  amount: string
}

const START: Row[] = [
  { id: 1, name: '방과후 기초Yap! 상', spec: '권', qty: '3', price: '10,000', amount: '30,000' },
  { id: 2, name: '방과후 기초Yap! 하', spec: '권', qty: '1', price: '10,000', amount: '10,000' },
  { id: 3, name: '10급Yap!', spec: '권', qty: '2', price: '10,000', amount: '20,000' },
]

function App() {
  const [rows, setRows] = useState<Row[]>(START)
  const [log, setLog] = useState<string[]>([])
  const [fresh, setFresh] = useState<number | null>(null)
  const body = useRef<HTMLTableSectionElement | null>(null)

  function commit(id: number, key: keyof Row, v: string) {
    setRows((rs) => rs.map((r) => (r.id === id ? { ...r, [key]: v } : r)))
    setLog((l) => [`저장: ${id}행 ${key} = ${JSON.stringify(v)}`, ...l].slice(0, 8))
  }

  function move(up: boolean) {
    const el = body.current
    if (!el) return
    const fields = Array.from(el.querySelectorAll<HTMLInputElement>('input[type="text"]'))
    const here = fields.indexOf(document.activeElement as HTMLInputElement)
    const to = nextFieldIndex(here, fields.length, up)
    setLog((l) => [`이동: ${here} → ${to === null ? '그대로' : to}`, ...l].slice(0, 8))
    if (to === null) return
    const t = fields[to]
    setTimeout(() => {
      t.focus()
      t.select()
    }, 0)
  }

  const cell = (r: Row, key: keyof Row, numeric = false) => (
    <td className={numeric ? 'num' : undefined}>
      <TextField
        numeric={numeric}
        value={r[key] as string}
        ariaLabel={`${r.id}-${key}`}
        autoFocus={key === 'name' && r.id === fresh}
        onCommitNext={move}
        onCommit={(v) => commit(r.id, key, v)}
      />
    </td>
  )

  return (
    <div className="main">
      <header>
        <h2>P4-4 키보드 입력 확인</h2>
        <p>Tab 으로 옆 칸, Enter 로 같은 칸의 다음 줄, Shift+Enter 로 윗줄, Esc 로 되돌리기.</p>
      </header>
      <div className="panel">
        <div className="table-wrap">
          <table>
            <thead>
              <tr>
                <th>물품명</th>
                <th style={{ width: 90 }}>규격</th>
                <th style={{ width: 80 }} className="num">수량</th>
                <th style={{ width: 100 }} className="num">단가</th>
                <th style={{ width: 110 }} className="num">금액</th>
              </tr>
            </thead>
            <tbody ref={body}>
              {rows.map((r) => (
                <tr key={r.id} className={r.id === 3 ? 'look' : ''}>
                  {cell(r, 'name')}
                  {cell(r, 'spec')}
                  {cell(r, 'qty', true)}
                  {cell(r, 'price', true)}
                  {cell(r, 'amount', true)}
                </tr>
              ))}
            </tbody>
          </table>
        </div>
        <div className="row" style={{ marginTop: 10 }}>
          <button
            className="small"
            onClick={() => {
              const id = Math.max(...rows.map((r) => r.id)) + 1
              setRows((rs) => [...rs, { id, name: '', spec: '', qty: '', price: '', amount: '' }])
              setFresh(id)
            }}
          >
            품목 줄 추가
          </button>
        </div>
      </div>
      <div className="panel">
        <h3>일어난 일</h3>
        <ul className="warn-list" id="log">
          {log.map((l, i) => (
            <li key={i} className="info">
              {l}
            </li>
          ))}
        </ul>
      </div>
    </div>
  )
}

createRoot(document.getElementById('root')!).render(<App />)
