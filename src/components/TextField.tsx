import { useEffect, useRef, useState } from 'react'
import { initialState, reduce } from '../lib/imeText'
import type { ImeEvent, ImeState } from '../lib/imeText'

interface Props {
  /** 바깥(서버)에서 온 값 */
  value: string
  /** **확정될 때만** 불린다 (초점을 잃거나 Enter). 치는 동안에는 안 부른다. */
  onCommit: (value: string) => void
  /**
   * 치는 동안 값을 알린다 (단추를 켜고 끄는 용도).
   * 여기서 받은 값을 그대로 `value` 로 되돌려 보내도 안전하다 —
   * 조합 중이거나 사용자가 고치는 중이면 상태 기계가 받아들이지 않는다.
   */
  onDraftChange?: (value: string) => void
  placeholder?: string
  disabled?: boolean
  className?: string
  style?: React.CSSProperties
  title?: string
  /** 숫자 칸처럼 오른쪽 정렬이 필요할 때 */
  numeric?: boolean
  ariaLabel?: string
  /** 손으로 더한 줄처럼, 나타나자마자 초점을 줘야 할 때 */
  autoFocus?: boolean
  /** Enter 를 눌러 확정한 뒤 다음 칸으로 넘어갈 때 */
  onCommitNext?: (shift: boolean) => void
}

/**
 * 한글 입력이 깨지지 않는 글자 입력칸.
 *
 * 보통의 controlled input 은 `value` 를 매번 다시 써 넣는데,
 * 한국어 IME 는 조합 중인 글자를 칸에 올려 두고 있어서 그때 값을 덮어쓰면 자모가 깨진다.
 * 이 컴포넌트는 **치는 동안에는 화면 값만 바꾸고**, 조합이 끝나고 초점을 잃을 때 한 번만 확정한다.
 * 규칙은 `lib/imeText.ts` 에 순수 함수로 있고 `scripts/check-ime.ts` 가 지킨다.
 */
export default function TextField({
  value,
  onCommit,
  placeholder,
  disabled,
  className,
  style,
  title,
  numeric,
  ariaLabel,
  autoFocus,
  onCommitNext,
  onDraftChange,
}: Props) {
  const [state, setState] = useState<ImeState>(() => initialState(value))
  // 최신 상태를 effect 안에서 읽기 위해 (의존성에 state 를 넣으면 매번 다시 돈다)
  const ref = useRef(state)
  ref.current = state

  function dispatch(event: ImeEvent) {
    const r = reduce(ref.current, event)
    ref.current = r.state
    setState(r.state)
    if (event.type === 'change' || event.type === 'compositionEnd') {
      onDraftChange?.(r.state.draft)
    }
    if (r.commit !== null) onCommit(r.commit)
  }

  // 부모가 값을 바꿨을 때만 알린다. 받아들일지는 상태 기계가 정한다.
  useEffect(() => {
    if (value !== ref.current.external) {
      dispatch({ type: 'external', value })
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [value])

  return (
    <input
      type="text"
      value={state.draft}
      disabled={disabled}
      placeholder={placeholder}
      className={numeric ? `num ${className ?? ''}`.trim() : className}
      style={style}
      title={title}
      aria-label={ariaLabel}
      autoFocus={autoFocus}
      onChange={(e) => dispatch({ type: 'change', value: e.target.value })}
      onCompositionStart={() => dispatch({ type: 'compositionStart' })}
      onCompositionEnd={(e) =>
        dispatch({ type: 'compositionEnd', value: e.currentTarget.value })
      }
      onFocus={() => dispatch({ type: 'focus' })}
      onBlur={() => dispatch({ type: 'blur' })}
      onKeyDown={(e) => {
        // 조합 중 키는 IME 의 것이다. 가로채면 한글이 깨진다.
        if (e.nativeEvent.isComposing) return
        if (e.key === 'Enter') {
          dispatch({ type: 'commitRequest' })
          onCommitNext?.(e.shiftKey)
        } else if (e.key === 'Escape') {
          dispatch({ type: 'cancel' })
        }
      }}
    />
  )
}
