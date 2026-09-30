import { useCallback, useEffect, useState } from 'react'
import * as ipc from '../ipc'
import TextField from '../components/TextField'
import AboutPanel from '../components/AboutPanel'
import type { Department, VendorUnit } from '../ipc/types'
import { defaultPhraseName } from '../lib/phrase'

interface Props {
  onError: (m: string) => void
}

const BLANK: Department = {
  id: 0,
  displayName: '',
  phraseName: '',
  sortOrder: 0,
  active: true,
  aliases: [],
  vendors: [],
}

export default function SetupStep({ onError }: Props) {
  const [depts, setDepts] = useState<Department[]>([])
  const [sel, setSel] = useState<number | null>(null)
  const [draft, setDraft] = useState<Department | null>(null)
  const [saved, setSaved] = useState('')

  const refresh = useCallback(
    async (keep?: number) => {
      try {
        const list = await ipc.listDepartments()
        setDepts(list)
        const id = keep ?? sel
        const found = list.find((d) => d.id === id) ?? list[0] ?? null
        setSel(found?.id ?? null)
        setDraft(found ? structuredClone(found) : null)
      } catch (e) {
        onError(ipc.errorMessage(e))
      }
    },
    [onError, sel],
  )

  useEffect(() => {
    void refresh()
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [])

  function pick(d: Department) {
    setSel(d.id)
    setDraft(structuredClone(d))
    setSaved('')
  }

  function addDept() {
    setSel(null)
    setDraft({ ...structuredClone(BLANK), sortOrder: depts.length })
    setSaved('')
  }

  async function save() {
    if (!draft) return
    try {
      const phraseName = draft.phraseName.trim() || defaultPhraseName(draft.displayName)
      const id = await ipc.saveDepartment({ ...draft, phraseName })
      setSaved('저장했습니다.')
      await refresh(id)
    } catch (e) {
      onError(ipc.errorMessage(e))
    }
  }

  async function remove() {
    if (!draft || draft.id === 0) return
    if (!confirm(`'${draft.displayName}' 부서를 지울까요? 정산 별칭과 거래처도 함께 지워집니다.`)) return
    try {
      await ipc.deleteDepartment(draft.id)
      setSel(null)
      await refresh()
    } catch (e) {
      onError(ipc.errorMessage(e))
    }
  }

  async function doExport() {
    try {
      const data = await ipc.exportSetup()
      const blob = new Blob([JSON.stringify(data, null, 2)], { type: 'application/json' })
      const a = document.createElement('a')
      a.href = URL.createObjectURL(blob)
      a.download = '부서설정.json'
      a.click()
      URL.revokeObjectURL(a.href)
    } catch (e) {
      onError(ipc.errorMessage(e))
    }
  }

  function doImport(file: File) {
    const reader = new FileReader()
    reader.onload = async () => {
      try {
        const data = JSON.parse(String(reader.result))
        if (!confirm('지금 등록된 부서 설정을 모두 지우고 파일 내용으로 바꿉니다. 계속할까요?')) return
        const n = await ipc.importSetup(data)
        setSaved(`${n}개 부서를 가져왔습니다.`)
        await refresh()
      } catch (e) {
        onError(ipc.errorMessage(e))
      }
    }
    reader.readAsText(file)
  }

  const set = <K extends keyof Department>(k: K, v: Department[K]) =>
    setDraft((d) => (d ? { ...d, [k]: v } : d))

  function setAlias(i: number, v: string) {
    setDraft((d) => {
      if (!d) return d
      const a = [...d.aliases]
      a[i] = v
      return { ...d, aliases: a }
    })
  }

  function setVendor(i: number, patch: Partial<VendorUnit>) {
    setDraft((d) => {
      if (!d) return d
      const v = [...d.vendors]
      v[i] = { ...v[i], ...patch }
      return { ...d, vendors: v }
    })
  }

  return (
    <>
      <header>
        <h2>1. 부서 설정</h2>
        <p>품의 부서 · 정산 별칭 · 거래처 관리 단위를 등록합니다. 셋은 서로 다른 개념입니다.</p>
      </header>

      {/* 프로그램 정보·업데이트 — 부서 설정과 상관없지만 여기가 이 앱의 설정 화면이다.
          부서가 많으면 아래쪽은 한참 굴려야 닿으므로 맨 위에 둔다. */}
      <AboutPanel />

      <div className="panel">
        <div className="row">
          <button onClick={addDept}>부서 추가</button>
          <div className="spacer" />
          <button className="small" onClick={doExport}>
            설정 내보내기(JSON)
          </button>
          <label className="small" style={{ display: 'inline-block' }}>
            <input
              type="file"
              accept="application/json,.json"
              style={{ display: 'none' }}
              onChange={(e) => {
                const f = e.target.files?.[0]
                if (f) doImport(f)
                e.target.value = ''
              }}
            />
            <span
              style={{
                display: 'inline-block',
                padding: '3px 8px',
                fontSize: 12,
                border: '1px solid var(--line)',
                borderRadius: 5,
                cursor: 'pointer',
                background: '#fff',
              }}
            >
              설정 가져오기(JSON)
            </span>
          </label>
        </div>
        {saved && <p className="note">{saved}</p>}
      </div>

      <div className="dept-grid">
        <div className="dept-list">
          {depts.length === 0 && <div className="empty" style={{ padding: 16 }}>등록된 부서가 없습니다.</div>}
          {depts.map((d) => (
            <button key={d.id} className={d.id === sel ? 'sel' : ''} onClick={() => pick(d)}>
              {d.displayName}
              <span className="note" style={{ marginTop: 2 }}>
                별칭 {d.aliases.length} · 거래처 {d.vendors.length}
              </span>
            </button>
          ))}
        </div>

        {draft ? (
          <div>
            <div className="panel">
              <h3>품의 부서</h3>
              <div className="row">
                <label className="field" style={{ flex: 1 }}>
                  품의 부서명
                  <TextField
                    key={`dn-${draft.id}`}
                    value={draft.displayName}
                    placeholder="예: 토탈공예미니어처"
                    onCommit={(v) => set('displayName', v)}
                  />
                </label>
                <label className="field" style={{ flex: 1 }}>
                  품의 표기명 (문구 앞에 붙습니다)
                  <TextField
                    key={`pn-${draft.id}`}
                    value={draft.phraseName}
                    placeholder={draft.displayName ? defaultPhraseName(draft.displayName) : '예: 토탈공예미니어처부'}
                    onCommit={(v) => set('phraseName', v)}
                  />
                </label>
              </div>
              <p className="note">
                품의 표기명은 모든 부서에 붙습니다. 예: 「{draft.phraseName || defaultPhraseName(draft.displayName || '○○')} 프로보테크닉 교구 외 1종」
              </p>
            </div>

            <div className="two-col">
              <div className="concept-box">
                <h4>정산 별칭</h4>
                <p className="why">
                  정산자료 「부서명」 칸에 적히는 이름입니다. 여기에 넣은 이름들의 금액이 이 부서로 <b>자동 합산</b>됩니다.
                  <br />
                  거래처와는 아무 관계가 없고, 견적서 파일명 매칭에도 쓰지 않습니다.
                </p>
                {draft.aliases.map((a, i) => (
                  <div className="chip-row" key={i}>
                    <TextField
                      key={`alias-${draft.id}-${i}`}
                      value={a}
                      style={{ flex: 1 }}
                      placeholder="예: 토탈공예미니어처1"
                      onCommit={(v) => setAlias(i, v)}
                    />
                    <button
                      className="small danger"
                      onClick={() =>
                        set(
                          'aliases',
                          draft.aliases.filter((_, k) => k !== i),
                        )
                      }
                    >
                      지우기
                    </button>
                  </div>
                ))}
                <button className="small" onClick={() => set('aliases', [...draft.aliases, ''])}>
                  별칭 추가
                </button>
              </div>

              <div className="concept-box">
                <h4>거래처 관리 단위</h4>
                <p className="why">
                  견적서 한 장 = 품의 한 줄입니다. <b>관리명을 견적서 파일명에 넣어</b> 주세요.
                  <br />
                  거래처가 하나면 부서명만 넣어도 되고, 여럿이면 관리명까지 넣습니다.
                </p>
                {draft.vendors.map((v, i) => (
                  <div key={i} style={{ marginBottom: 10 }}>
                    <div className="chip-row">
                      <TextField
                        key={`vm-${draft.id}-${i}`}
                        value={v.mgmtName}
                        style={{ flex: 1 }}
                        placeholder="관리명 (예: 로봇과학1)"
                        onCommit={(val) => setVendor(i, { mgmtName: val })}
                      />
                      <button
                        className="small danger"
                        onClick={() =>
                          set(
                            'vendors',
                            draft.vendors.filter((_, k) => k !== i),
                          )
                        }
                      >
                        지우기
                      </button>
                    </div>
                    <TextField
                      key={`vn-${draft.id}-${i}`}
                      style={{ width: '100%' }}
                      value={v.vendorName}
                      placeholder="거래처 이름 (선택, 참고용)"
                      onCommit={(val) => setVendor(i, { vendorName: val })}
                    />
                  </div>
                ))}
                <button
                  className="small"
                  onClick={() =>
                    set('vendors', [
                      ...draft.vendors,
                      {
                        id: 0,
                        mgmtName: '',
                        vendorName: '',
                        note: '',
                        sortOrder: draft.vendors.length,
                        active: true,
                      },
                    ])
                  }
                >
                  거래처 추가
                </button>
              </div>
            </div>

            <div className="panel row" style={{ marginTop: 16 }}>
              <button className="primary" onClick={save}>
                저장
              </button>
              {draft.id > 0 && (
                <button className="danger" onClick={remove}>
                  이 부서 지우기
                </button>
              )}
            </div>
          </div>
        ) : (
          <div className="empty">왼쪽에서 부서를 고르거나 「부서 추가」를 누르세요.</div>
        )}
      </div>
    </>
  )
}
