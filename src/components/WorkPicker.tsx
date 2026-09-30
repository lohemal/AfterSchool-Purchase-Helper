import { useRef, useState } from 'react'
import * as ipc from '../ipc'
import type { Work } from '../ipc/types'
import TextField from './TextField'
import {
  cleanWorkName,
  defaultWorkName,
  isValidWorkName,
  monthOf,
  schoolYearOf,
  workNameError,
} from '../lib/workName'

/** 무엇을 하고 있나. `null` 이면 고르기만 한다. */
type Mode = null | 'new' | 'rename' | 'delete'

interface Props {
  works: Work[]
  workId: number | null
  onSelect: (id: number | null) => void
  /**
   * 작업이 만들어졌거나·이름이 바뀌었거나·지워진 뒤.
   * 목록을 다시 읽는 일은 부모가 한다.
   */
  onChanged: (action: 'created' | 'renamed' | 'deleted', id: number | null) => void | Promise<void>
  onError: (message: string) => void
}

/**
 * 작업 고르기와 작업 관리(만들기·이름 바꾸기·지우기).
 *
 * ## 이름을 물어보는 칸을 화면 안에 둔다
 * `window.prompt()` 는 쓰지 않는다. 이 앱이 도는 WebView2 에는 `prompt` 가 없어서
 * 아무 말 없이 `null` 이 돌아온다. 확인 창도 마찬가지 까닭과, 지울 때 무엇이
 * 함께 지워지는지 여러 줄로 보여 주기 위해 화면 안에 그린다.
 *
 * ## 지우기는 한 번 더 묻는다
 * 작업을 지우면 그 작업의 견적서·추출 결과·정산·배분·검증·생성 기록이 함께 사라진다.
 * 되돌릴 수 없으므로 무엇이 사라지는지 적어 두고 묻는다.
 * **원본 견적서 파일과 만들어 둔 Excel 은 지우지 않는다.**
 */
export default function WorkPicker({ works, workId, onSelect, onChanged, onError }: Props) {
  const [mode, setMode] = useState<Mode>(null)
  const [name, setName] = useState('')
  const [saving, setSaving] = useState(false)
  // Enter 로 바로 확정할 때 최신 값을 읽으려면 ref 가 필요하다
  // (React 상태는 같은 이벤트 안에서 아직 바뀌지 않았을 수 있다)
  const nameRef = useRef('')

  const work = works.find((w) => w.id === workId) ?? null

  function setDraft(value: string) {
    nameRef.current = value
    setName(value)
  }

  function close() {
    setMode(null)
    setDraft('')
    setSaving(false)
  }

  function startNew() {
    setDraft(defaultWorkName(new Date()))
    setMode('new')
  }

  function startRename() {
    if (!work) return
    setDraft(work.title)
    setMode('rename')
  }

  async function submitNew() {
    const title = cleanWorkName(nameRef.current)
    const problem = workNameError(title)
    if (problem) {
      onError(problem)
      return
    }
    const now = new Date()
    setSaving(true)
    try {
      // 학년도·월·종류는 지금까지와 같은 방식으로 정한다. 바꾼 것은 **이름뿐**이다.
      const id = await ipc.createWork(title, schoolYearOf(now), monthOf(now), '교재비')
      close()
      await onChanged('created', id)
    } catch (e) {
      setSaving(false)
      onError(ipc.errorMessage(e))
    }
  }

  async function submitRename() {
    if (!work) return
    const title = cleanWorkName(nameRef.current)
    const problem = workNameError(title)
    if (problem) {
      onError(problem)
      return
    }
    if (title === work.title) {
      close()
      return
    }
    setSaving(true)
    try {
      await ipc.renameWork(work.id, title)
      close()
      await onChanged('renamed', work.id)
    } catch (e) {
      setSaving(false)
      onError(ipc.errorMessage(e))
    }
  }

  async function submitDelete() {
    if (!work) return
    setSaving(true)
    try {
      await ipc.deleteWork(work.id)
      close()
      await onChanged('deleted', null)
    } catch (e) {
      setSaving(false)
      onError(ipc.errorMessage(e))
    }
  }

  const canSave = isValidWorkName(name) && !saving

  return (
    <div className="work-pick">
      <label className="field">
        작업
        <select
          value={workId ?? ''}
          disabled={mode !== null}
          onChange={(e) => onSelect(e.target.value === '' ? null : Number(e.target.value))}
        >
          {works.length === 0 && <option value="">작업 없음</option>}
          {works.map((w) => (
            <option key={w.id} value={w.id}>
              {w.title}
            </option>
          ))}
        </select>
      </label>

      {mode === null && (
        <div className="work-actions">
          <button className="small" onClick={startNew}>
            새 작업 만들기
          </button>
          {work && (
            <>
              <button className="small" onClick={startRename}>
                이름 변경
              </button>
              <button className="small danger" onClick={() => setMode('delete')}>
                삭제
              </button>
            </>
          )}
        </div>
      )}

      {(mode === 'new' || mode === 'rename') && (
        <div className="work-form">
          <label className="field">
            작업명
            <TextField
              key={mode + String(workId)}
              value={name}
              autoFocus
              placeholder="예: 2026학년도 9월 교재비"
              onDraftChange={setDraft}
              onCommit={setDraft}
              onCommitNext={() => void (mode === 'new' ? submitNew() : submitRename())}
              ariaLabel="작업명"
            />
          </label>
          {!isValidWorkName(name) && <p className="work-warn">작업 이름을 입력해 주세요.</p>}
          <div className="work-actions">
            <button
              className="small primary"
              disabled={!canSave}
              onClick={() => void (mode === 'new' ? submitNew() : submitRename())}
            >
              {mode === 'new' ? '만들기' : '저장'}
            </button>
            <button className="small" disabled={saving} onClick={close}>
              취소
            </button>
          </div>
          {mode === 'rename' && (
            <p className="note">이름만 바뀝니다. 등록한 견적서와 정산·배분 자료는 그대로입니다.</p>
          )}
        </div>
      )}

      {mode === 'delete' && work && (
        <div className="work-form danger-box" role="alertdialog" aria-label="작업 삭제 확인">
          <p className="work-confirm">
            <b>{work.title}</b> 작업을 삭제하시겠습니까?
            <br />
            작업에 포함된 견적서, 정산 및 배분 자료도 함께 삭제됩니다.
          </p>
          <div className="work-actions">
            <button className="small" disabled={saving} onClick={close}>
              취소
            </button>
            <button className="small danger" disabled={saving} onClick={() => void submitDelete()}>
              삭제
            </button>
          </div>
          <p className="note">원본 견적서 파일과 만들어 둔 Excel 은 지우지 않습니다.</p>
        </div>
      )}
    </div>
  )
}
