import { useCallback, useRef, useState } from 'react'
import type { QuotePreview } from '../ipc/types'
import { PREVIEW_TIMEOUT_MS, type PreviewState } from './preview'
import { isTimeout, withTimeout } from './timeout'

/**
 * 원본 보기의 열고 닫기와 불러오기를 한곳에서 다룬다.
 *
 * **왜 훅으로 뺐나** — 예전에는 이 일을 `useEffect` 안에서 했다.
 * "불러오는 중" 상태를 켜면 그 상태가 effect 의 의존성에 들어 있어 effect 가 다시 돌았고,
 * 그때 앞선 요청의 정리(cleanup)가 먼저 실행돼 **이미 날아간 요청의 답을 버렸다.**
 * 그래서 화면이 "불러오는 중…" 에서 영영 내려오지 않았다.
 * 이제 불러오기는 **누를 때 부르는 보통 함수**이고, 늦게 온 답은 번호로 가린다.
 *
 * 규칙
 *   - `loading` 은 반드시 `ready` 나 `failed` 로 끝난다. 중간에 갇히지 않는다.
 *   - 제한 시간이 지나면 실패로 본다(원래 일은 못 멈추지만 화면은 기다리지 않는다).
 *   - 빠르게 여러 번 누르거나 다른 견적서로 옮기면 **늦게 온 옛 답은 버린다.**
 */
export function useSourcePreview(
  load: (quoteId: number) => Promise<QuotePreview>,
  timeoutMs: number = PREVIEW_TIMEOUT_MS,
) {
  const [open, setOpen] = useState(false)
  const [state, setState] = useState<PreviewState>({ kind: 'idle' })
  /** 몇 번째 요청인지 — 늦게 온 옛 답을 가려내는 표 */
  const seq = useRef(0)

  const start = useCallback(
    async (quoteId: number) => {
      const mine = ++seq.current
      setState({ kind: 'loading' })
      try {
        const data = await withTimeout(load(quoteId), timeoutMs)
        if (seq.current !== mine) return
        setState({ kind: 'ready', data })
      } catch (e) {
        if (seq.current !== mine) return
        const message = isTimeout(e)
          ? '원본을 불러오는 데 시간이 너무 오래 걸립니다.'
          : errorText(e)
        setState({ kind: 'failed', message })
      }
    },
    [load, timeoutMs],
  )

  const toggle = useCallback(
    (quoteId: number) => {
      if (open) {
        setOpen(false)
        return
      }
      setOpen(true)
      void start(quoteId)
    },
    [open, start],
  )

  /** 다른 견적서로 옮겼을 때 — 접고 처음 상태로. 날아간 요청의 답은 버린다. */
  const reset = useCallback(() => {
    seq.current += 1
    setOpen(false)
    setState({ kind: 'idle' })
  }, [])

  return { open, state, toggle, retry: start, reset }
}

function errorText(e: unknown): string {
  if (e && typeof e === 'object' && 'message' in e) return String((e as { message: unknown }).message)
  return String(e)
}
