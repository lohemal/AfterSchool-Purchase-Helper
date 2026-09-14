import type { QuotePreview } from '../ipc/types'

/**
 * 원본 보기의 상태. **네 가지뿐이고, 그 사이에 갇히는 자리가 없다.**
 *
 *   idle    → 아직 누르지 않았다
 *   loading → 불러오는 중 (반드시 ready 나 failed 로 끝난다)
 *   ready   → 그림이 있거나(pages), 그림이 없는 형식이라는 안내(note)
 *   failed  → 못 불러왔다. 까닭과 함께 `원본 파일 열기` 로 안내한다
 */
export type PreviewState =
  | { kind: 'idle' }
  | { kind: 'loading' }
  | { kind: 'ready'; data: QuotePreview }
  | { kind: 'failed'; message: string }

/** 원본 보기를 이보다 오래 기다리지 않는다 */
export const PREVIEW_TIMEOUT_MS = 20_000

/** 화면에 그림을 실제로 그릴 수 있는가 */
export function hasPicture(s: PreviewState): boolean {
  return s.kind === 'ready' && s.data.pages.length > 0
}

/**
 * 사람에게 보여 줄 한 줄.
 *
 * **어떤 상태에서도 빈 문자열이 나오지 않는다** — 사용자를 말없이 기다리게 두지 않기 위해서다.
 * 그림이 있는 경우에만 빈 문자열이고, 그때는 그림 자체가 답이다.
 */
export function previewMessage(s: PreviewState): string {
  switch (s.kind) {
    case 'idle':
      return ''
    case 'loading':
      return '원본을 불러오는 중입니다…'
    case 'ready':
      return s.data.pages.length > 0 ? '' : s.data.note
    case 'failed':
      return `${s.message} 아래 ‘원본 파일 열기’ 로 확인해 주세요.`
  }
}

/** 이 상태에서 `원본 파일 열기` 를 권해야 하는가 */
export function shouldOfferOpenFile(s: PreviewState): boolean {
  return s.kind === 'failed' || (s.kind === 'ready' && s.data.pages.length === 0)
}
