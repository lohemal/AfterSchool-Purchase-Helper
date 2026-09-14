/**
 * 원본 보기 확인용 화면. **제품 코드가 아니다.**
 *
 * 제품과 **똑같은 훅**(`useSourcePreview`)을 쓰고 불러오는 함수만 흉내 낸다.
 * 사람이 겪었던 "불러오는 중…" 에서 안 내려오는 문제가 정말 사라졌는지,
 * 느릴 때·실패할 때·빠르게 여러 번 누를 때 화면이 어떻게 끝나는지 본다.
 *
 * 띄우기: npx vite --port 5176 → http://localhost:5176/test/ui/preview.html
 */
import { useState } from 'react'
import { createRoot } from 'react-dom/client'
import { useSourcePreview } from '../../src/lib/useSourcePreview'
import { hasPicture, previewMessage, shouldOfferOpenFile } from '../../src/lib/preview'
import type { QuotePreview } from '../../src/ipc/types'

/** 1x1 짜리 빨간 점 (진짜 JPEG) */
const DOT =
  '/9j/4AAQSkZJRgABAQEAYABgAAD/2wBDAAgGBgcGBQgHBwcJCQgKDBQNDAsLDBkSEw8UHRofHh0a' +
  'HBwgJC4nICIsIxwcKDcpLDAxNDQ0Hyc5PTgyPC4zNDL/wAALCAABAAEBAREA/8QAFAABAAAAAAAA' +
  'AAAAAAAAAAAACf/EABQQAQAAAAAAAAAAAAAAAAAAAAD/2gAIAQEAAD8AKp//2Q=='

function page(label: string): QuotePreview {
  return {
    pages: [{ label, mime: 'image/jpeg', base64: DOT, width: 160, height: 160 }],
    note: '',
  }
}
const NO_PICTURE: QuotePreview = {
  pages: [],
  note: '글자로 된 PDF 라 미리 보여 줄 그림이 없습니다. 품목의 원본 위치에 적힌 쪽을 보시고, 아래 단추로 원본을 열어 확인해 주세요.',
}

type Mode = 'fast' | 'slow' | 'hang' | 'error' | 'nopicture'

function App() {
  const [mode, setMode] = useState<Mode>('fast')
  const [quoteId, setQuoteId] = useState(1)
  const [calls, setCalls] = useState(0)

  // 불러오기만 흉내 낸다. 훅은 제품과 같은 것을 쓴다.
  async function load(id: number): Promise<QuotePreview> {
    setCalls((n) => n + 1)
    const wait = (ms: number) => new Promise((r) => setTimeout(r, ms))
    if (mode === 'fast') {
      await wait(80)
      return page(`${id}번 견적서 원본`)
    }
    if (mode === 'nopicture') {
      await wait(80)
      return NO_PICTURE
    }
    if (mode === 'slow') {
      await wait(1500)
      return page(`${id}번 (느리게 옴)`)
    }
    if (mode === 'error') {
      await wait(80)
      throw { message: '원본 파일을 열지 못했습니다.' }
    }
    await wait(600_000) // hang — 시간 제한이 잡아야 한다
    return page('영영 안 옴')
  }

  // 시간 제한을 1초로 줄여 바로 확인할 수 있게 한다 (제품은 20초)
  const src = useSourcePreview(load, 1000)

  return (
    <div className="main">
      <header>
        <h2>원본 보기 확인</h2>
        <p>
          상태: <b id="state">{src.state.kind}</b> · 펼침 <b id="open">{String(src.open)}</b> · 불러온
          횟수 <b id="calls">{calls}</b>
        </p>
      </header>

      <div className="panel">
        <div className="row">
          {(['fast', 'slow', 'hang', 'error', 'nopicture'] as Mode[]).map((m) => (
            <button
              key={m}
              id={`mode-${m}`}
              className={m === mode ? 'small primary' : 'small'}
              onClick={() => setMode(m)}
            >
              {m}
            </button>
          ))}
          <div className="spacer" />
          <button
            id="next-quote"
            className="small"
            onClick={() => {
              setQuoteId((n) => n + 1)
              src.reset()
            }}
          >
            다른 견적서로 ({quoteId})
          </button>
        </div>
      </div>

      <div className="panel">
        <div className="row">
          <button id="toggle" className="small" onClick={() => src.toggle(quoteId)}>
            {src.open ? '원본 접기' : '원본 보기'}
          </button>
        </div>

        {src.open && (
          <div style={{ marginTop: 12 }}>
            {previewMessage(src.state) !== '' && (
              <p
                id="message"
                className="note"
                style={src.state.kind === 'failed' ? { color: 'var(--bad)' } : undefined}
              >
                {previewMessage(src.state)}
              </p>
            )}
            {shouldOfferOpenFile(src.state) && (
              <div className="row">
                <button id="open-file" className="small">
                  원본 파일 열기
                </button>
                {src.state.kind === 'failed' && (
                  <button id="retry" className="small" onClick={() => void src.retry(quoteId)}>
                    다시 시도
                  </button>
                )}
              </div>
            )}
            {hasPicture(src.state) &&
              src.state.kind === 'ready' &&
              src.state.data.pages.map((p) => (
                <figure key={p.label} className="source-page">
                  <figcaption>{p.label}</figcaption>
                  <img
                    id="pic"
                    src={`data:${p.mime};base64,${p.base64}`}
                    alt={p.label}
                    width={p.width}
                    height={p.height}
                  />
                </figure>
              ))}
          </div>
        )}
      </div>
    </div>
  )
}

createRoot(document.getElementById('root')!).render(<App />)
