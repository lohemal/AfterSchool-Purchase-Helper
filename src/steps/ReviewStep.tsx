import { useCallback, useEffect, useRef, useState } from 'react'
import * as ipc from '../ipc'
import TextField from '../components/TextField'
import type {
  CompareBasis,
  ItemRow,
  QuoteRow,
  RowKind,
  SignEffect,
} from '../ipc/types'
import { ROW_KIND_LABEL, SIGN_LABEL } from '../ipc/types'
import { comma, parseMoney } from '../lib/money'
import { SOURCE_LABEL, TRUST_BADGE, TRUST_LABEL } from '../lib/quoteTrust'
import { countNeedsLook, isBlank, lookReasons, needsLook } from '../lib/itemStatus'
import { nextFieldIndex } from '../lib/gridKeys'
import { hasPicture, previewMessage, shouldOfferOpenFile } from '../lib/preview'
import { useSourcePreview } from '../lib/useSourcePreview'

interface Props {
  workId: number
  onError: (m: string) => void
}

export default function ReviewStep({ workId, onError }: Props) {
  const [quotes, setQuotes] = useState<QuoteRow[]>([])
  const [sel, setSel] = useState<number | null>(null)
  // 방금 손으로 더한 줄 — 그 줄의 물품명 칸으로 초점을 옮긴다
  const [fresh, setFresh] = useState<number | null>(null)
  const tableRef = useRef<HTMLTableSectionElement | null>(null)
  // 원본 보기 (P4-2) — 열고 닫기·불러오기·시간 제한은 훅 한곳에 있다
  const src = useSourcePreview(ipc.quotePreview)

  const refresh = useCallback(async () => {
    try {
      const list = await ipc.listQuotes(workId)
      setQuotes(list)
      setSel((cur) => (cur !== null && list.some((q) => q.id === cur) ? cur : list[0]?.id ?? null))
    } catch (e) {
      onError(ipc.errorMessage(e))
    }
  }, [workId, onError])

  useEffect(() => {
    void refresh()
  }, [refresh])

  const q = quotes.find((x) => x.id === sel) ?? null

  // 다른 견적서를 고르면 원본 보기를 접고 처음 상태로 돌린다
  const resetSource = src.reset
  useEffect(() => {
    resetSource()
  }, [sel, resetSource])

  async function saveItem(item: ItemRow, patch: Partial<ItemRow>) {
    const merged = { ...item, ...patch }
    try {
      await ipc.updateItem({
        id: merged.id,
        kind: merged.kind,
        signEffect: merged.signEffect,
        displayName: merged.displayName,
        spec: merged.spec,
        qty: merged.qty,
        unitPrice: merged.unitPrice,
        amount: merged.amount,
      })
      await refresh()
    } catch (e) {
      onError(ipc.errorMessage(e))
    }
  }

  async function act(fn: () => Promise<unknown>) {
    try {
      await fn()
      await refresh()
    } catch (e) {
      onError(ipc.errorMessage(e))
    }
  }

  /** 품목 줄을 더하고 **바로 물품명 칸으로 초점을 옮긴다** (손으로 채우는 흐름이 끊기지 않게) */
  async function addRow() {
    try {
      const id = await ipc.addItem(q!.id)
      setFresh(id)
      await refresh()
    } catch (e) {
      onError(ipc.errorMessage(e))
    }
  }

  /**
   * Enter 를 누르면 **같은 칸의 다음 줄**로 내려간다 (Shift+Enter 는 위로).
   * 값을 여러 줄 이어서 고칠 때 마우스를 다시 잡지 않아도 된다.
   */
  function moveToNextRow(up: boolean) {
    const body = tableRef.current
    if (!body) return
    const fields = Array.from(body.querySelectorAll<HTMLInputElement>('input[type="text"]'))
    const here = fields.indexOf(document.activeElement as HTMLInputElement)
    const to = nextFieldIndex(here, fields.length, up)
    if (to === null) return
    // 확정 저장이 끝난 뒤 옮긴다 (다시 그리는 동안 초점이 튀지 않게)
    const target = fields[to]
    setTimeout(() => {
      target.focus()
      target.select()
    }, 0)
  }

  const itemCount = q?.items.filter((i) => i.kind === 'item').length ?? 0
  const lookCount = q ? countNeedsLook(q.items) : 0
  const blankCount = q ? q.items.filter(isBlank).length : 0

  return (
    <>
      <header>
        <h2>3. 추출 결과 확인</h2>
        <p>
          읽어 온 값을 확인하고 고칩니다. 견적서 원문은 회색 글씨로 그대로 남아 있으며 절대 바뀌지
          않습니다.
        </p>
      </header>

      {quotes.length === 0 ? (
        <div className="empty">먼저 견적서를 등록해 주세요.</div>
      ) : (
        <div className="review">
          <div className="quote-list">
            {quotes.map((x) => (
              <button key={x.id} className={x.id === sel ? 'sel' : ''} onClick={() => setSel(x.id)}>
                <span className="qname">{x.vendorMgmtName || x.sourceName}</span>
                <span className="note" style={{ marginTop: 2 }}>
                  {x.parseStatus === 'ok'
                    ? `품목 ${x.items.filter((i) => i.kind === 'item').length}종 · ${comma(x.compareTotal)}원`
                    : '읽지 못함 — 직접 입력'}
                </span>
                {x.parseStatus === 'ok' && countNeedsLook(x.items) > 0 && (
                  <span className="badge check" style={{ marginTop: 4 }}>
                    확인할 줄 {countNeedsLook(x.items)}개
                  </span>
                )}
                {x.parseStatus === 'ok' && x.source !== 'structured' && x.trust !== '' && (
                  <span className={`badge ${TRUST_BADGE[x.trust]}`} style={{ marginTop: 4 }}>
                    {TRUST_LABEL[x.trust]}
                  </span>
                )}
              </button>
            ))}
          </div>

          {q && (
            <div>
              <div className="panel">
                <h3>
                  {q.vendorMgmtName || '(거래처 미지정)'}
                  <span className="sub">{q.sourceName}</span>
                </h3>
                {q.source !== 'structured' && q.parseStatus === 'ok' && (
                  <div className="banner notice" style={{ marginTop: 8 }}>
                    <b>
                      {SOURCE_LABEL[q.source]} · {TRUST_LABEL[q.trust] || '확인 필요'}
                    </b>
                    <div className="raw">
                      프로그램이 값을 고치지 않았습니다. 아래 표를 원본과 맞춰 본 뒤 넘어가 주세요.
                      원본 위치 칸에 몇 쪽에서 읽었는지 적혀 있습니다.
                    </div>
                  </div>
                )}
                {q.tableNote && <p className="note">{q.tableNote}</p>}
                {q.vendorNameInDoc && (
                  <p className="note">견적서에 적힌 상호: {q.vendorNameInDoc} (참고용)</p>
                )}
                {q.warnings.length > 0 && (
                  <ul className="warn-list">
                    {q.warnings.map((w, i) => (
                      <li key={i} className={w.severity}>
                        {w.message}
                      </li>
                    ))}
                  </ul>
                )}

                <div className="totals" style={{ marginTop: 12 }}>
                  <div>
                    <span>견적서 공급가액</span>
                    <b>{q.supplyTotal === null ? '—' : comma(q.supplyTotal)}</b>
                  </div>
                  <div>
                    <span>세액</span>
                    <b>{q.taxTotal === null ? '—' : comma(q.taxTotal)}</b>
                  </div>
                  <div>
                    <span>견적서 합계금액</span>
                    <b>{q.grandTotal === null ? '—' : comma(q.grandTotal)}</b>
                    {q.rawGrandTotal && q.source === 'ocr' && (
                      <span className="raw">사진에서 읽은 글자: {q.rawGrandTotal}</span>
                    )}
                  </div>
                  <div>
                    <span>품목 합</span>
                    <b>{comma(q.itemSum)}</b>
                  </div>
                  <div>
                    <span>조정 합</span>
                    <b>{comma(q.adjustmentSum)}</b>
                  </div>
                  <div>
                    <span>계산 총액</span>
                    <b>{comma(q.computedTotal)}</b>
                  </div>
                </div>

                <div className="row" style={{ marginTop: 12 }}>
                  <label className="field">
                    정산과 비교할 값
                    <select
                      value={q.compareBasis}
                      onChange={(e) =>
                        act(() => ipc.setCompareBasis(q.id, e.target.value as CompareBasis))
                      }
                    >
                      <option value="grand">견적서 합계금액(공급가액+세액)</option>
                      <option value="supply">견적서 공급가액</option>
                      <option value="computedTotal">프로그램이 더한 값</option>
                    </select>
                  </label>
                  <div style={{ alignSelf: 'flex-end', paddingBottom: 4 }}>
                    → <b>{comma(q.compareTotal)}</b>원
                  </div>
                  <div className="spacer" />
                  {q.sourceExists && (
                    <>
                      <button className="small" onClick={() => src.toggle(q.id)}>
                        {src.open ? '원본 접기' : '원본 보기'}
                      </button>
                      <button
                        className="small"
                        title={q.sourcePath}
                        onClick={() =>
                          ipc.openSourceFile(q.sourcePath).catch((e) => onError(ipc.errorMessage(e)))
                        }
                      >
                        원본 파일 열기
                      </button>
                      <button className="small" onClick={() => act(() => ipc.reparseQuote(q.id))}>
                        원본에서 다시 읽기
                      </button>
                    </>
                  )}
                </div>
                {!q.sourceExists && (
                  <p className="note">
                    원본 파일이 있던 자리에 없습니다. 읽어 둔 내용은 그대로 쓸 수 있습니다.
                  </p>
                )}
              </div>

              {src.open && q.sourceExists && (
                <div className="panel">
                  <h3>
                    원본<span className="sub">{q.sourceName}</span>
                  </h3>

                  {/* 어떤 상태에서도 화면에 할 말이 있다 — 말없이 기다리게 두지 않는다 */}
                  {previewMessage(src.state) !== '' && (
                    <p
                      className="note"
                      style={src.state.kind === 'failed' ? { color: 'var(--bad)' } : undefined}
                    >
                      {previewMessage(src.state)}
                    </p>
                  )}

                  {shouldOfferOpenFile(src.state) && (
                    <div className="row">
                      <button
                        className="small"
                        onClick={() =>
                          ipc
                            .openSourceFile(q.sourcePath)
                            .catch((e) => onError(ipc.errorMessage(e)))
                        }
                      >
                        원본 파일 열기
                      </button>
                      {src.state.kind === 'failed' && (
                        <button className="small" onClick={() => void src.retry(q.id)}>
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
                          src={`data:${p.mime};base64,${p.base64}`}
                          alt={`${q.sourceName} ${p.label}`}
                          width={p.width}
                          height={p.height}
                        />
                      </figure>
                    ))}
                </div>
              )}

              <div className="panel">
                <h3>
                  품목<span className="sub">일반 품목 {itemCount}종 — 문구의 「N종」은 이 수입니다</span>
                </h3>
                {lookCount > 0 && (
                  <p className="note" style={{ marginBottom: 8, color: 'var(--check)' }}>
                    노란 줄 <b>{lookCount}개</b>를 원본과 맞춰 봐 주세요. 값을 고치거나, 견적서에 없는
                    줄이면 오른쪽 <b>삭제</b>를 누르면 됩니다.
                  </p>
                )}
                {blankCount > 0 && (
                  <p className="note" style={{ marginBottom: 8, color: 'var(--check)' }}>
                    비어 있는 줄이 {blankCount}개 있습니다. 채우거나 지워 주세요 — 빈 줄도 「N종」에
                    들어갑니다.
                  </p>
                )}
                <div className="table-wrap">
                  <table>
                    <thead>
                      <tr>
                        <th style={{ width: 34 }}>대표</th>
                        <th style={{ width: 110 }}>종류</th>
                        <th>물품명 (품의 표시명 / 원문)</th>
                        <th style={{ width: 90 }}>규격</th>
                        <th style={{ width: 80 }} className="num">수량</th>
                        <th style={{ width: 100 }} className="num">단가</th>
                        <th style={{ width: 110 }} className="num">금액</th>
                        <th style={{ width: 90 }}>원본 위치</th>
                        <th style={{ width: 50 }}></th>
                      </tr>
                    </thead>
                    <tbody ref={tableRef}>
                      {q.items.map((it) => (
                        <tr
                          key={it.id}
                          className={[it.kind === 'item' ? '' : 'dim', needsLook(it) ? 'look' : '']
                            .filter(Boolean)
                            .join(' ')}
                        >
                          <td style={{ textAlign: 'center' }}>
                            <input
                              type="radio"
                              name={`rep-${q.id}`}
                              disabled={it.kind !== 'item'}
                              checked={q.representativeItemId === it.id}
                              onChange={() => act(() => ipc.setRepresentative(q.id, it.id))}
                            />
                          </td>
                          <td>
                            <select
                              value={it.kind}
                              onChange={(e) =>
                                saveItem(it, {
                                  kind: e.target.value as RowKind,
                                  signEffect:
                                    e.target.value === 'adjustment'
                                      ? (it.signEffect ?? 'unknown')
                                      : null,
                                })
                              }
                            >
                              {(['item', 'adjustment', 'zero'] as RowKind[]).map((k) => (
                                <option key={k} value={k}>
                                  {ROW_KIND_LABEL[k]}
                                </option>
                              ))}
                            </select>
                            {it.kind === 'adjustment' && (
                              <select
                                style={{ marginTop: 4, width: '100%' }}
                                value={it.signEffect ?? 'unknown'}
                                onChange={(e) =>
                                  saveItem(it, { signEffect: e.target.value as SignEffect })
                                }
                              >
                                {(['asWritten', 'subtract', 'unknown'] as SignEffect[]).map((s) => (
                                  <option key={s} value={s}>
                                    {SIGN_LABEL[s]}
                                  </option>
                                ))}
                              </select>
                            )}
                            {it.kind === 'zero' && <span className="raw">참고 · 금액 0</span>}
                          </td>
                          <td>
                            {/* 한글이 깨지지 않도록 확정될 때만 저장한다 (lib/imeText.ts) */}
                            <TextField
                              value={it.displayName}
                              ariaLabel="품의 표시명"
                              autoFocus={it.id === fresh}
                              onCommitNext={moveToNextRow}
                              onCommit={(v) => saveItem(it, { displayName: v })}
                            />
                            {lookReasons(it, q.source).map((r, i) => (
                              <span key={i} className="raw" style={{ color: 'var(--check)' }}>
                                {r}
                              </span>
                            ))}
                            {it.rawName !== '' && (
                              <span className="raw">
                                원문: {it.rawName}
                                {it.displayName !== it.rawName && (
                                  <button
                                    className="small"
                                    style={{ marginLeft: 6, padding: '0 5px' }}
                                    onClick={() => saveItem(it, { displayName: it.rawName })}
                                  >
                                    되돌리기
                                  </button>
                                )}
                              </span>
                            )}
                          </td>
                          <td>
                            <TextField
                              value={it.spec}
                              ariaLabel="규격"
                              onCommitNext={moveToNextRow}
                              onCommit={(v) => saveItem(it, { spec: v })}
                            />
                            {it.rawSpec !== '' && it.rawSpec !== it.spec && (
                              <span className="raw">원문: {it.rawSpec}</span>
                            )}
                          </td>
                          <td className="num">
                            <TextField
                              numeric
                              ariaLabel="수량"
                              value={it.qty === null ? '' : String(it.qty)}
                              onCommitNext={moveToNextRow}
                              onCommit={(v) => saveItem(it, { qty: parseMoney(v) })}
                            />
                            {it.rawQty !== '' && <span className="raw">원문: {it.rawQty}</span>}
                          </td>
                          <td className="num">
                            <TextField
                              numeric
                              ariaLabel="단가"
                              value={it.unitPrice === null ? '' : comma(it.unitPrice)}
                              onCommitNext={moveToNextRow}
                              onCommit={(v) => saveItem(it, { unitPrice: parseMoney(v) })}
                            />
                            {it.rawUnitPrice !== '' && (
                              <span className="raw">원문: {it.rawUnitPrice}</span>
                            )}
                          </td>
                          <td className="num">
                            <TextField
                              numeric
                              ariaLabel="금액"
                              value={it.amount === null ? '' : comma(it.amount)}
                              onCommitNext={moveToNextRow}
                              onCommit={(v) => saveItem(it, { amount: parseMoney(v) })}
                            />
                            {it.rawAmount !== '' && (
                              <span className="raw">원문: {it.rawAmount}</span>
                            )}
                            {it.warnings.map((w, i) => (
                              <span
                                key={i}
                                className="raw"
                                style={{ color: w.severity === 'warn' ? 'var(--check)' : undefined }}
                              >
                                {w.message}
                              </span>
                            ))}
                          </td>
                          <td className="cellref">{it.cellRef}</td>
                          <td>
                            <button
                              className="small danger"
                              onClick={() => act(() => ipc.deleteItem(it.id))}
                            >
                              삭제
                            </button>
                          </td>
                        </tr>
                      ))}
                    </tbody>
                  </table>
                </div>
                <div className="row" style={{ marginTop: 10 }}>
                  <button className="small" onClick={addRow}>
                    품목 줄 추가
                  </button>
                  <span className="note" style={{ margin: 0 }}>
                    Tab 으로 옆 칸, Enter 로 같은 칸의 다음 줄로 갑니다. Esc 를 누르면 고치기 전
                    값으로 돌아갑니다.
                  </span>
                </div>
              </div>

              <div className="panel">
                <h3>품의 내용 문구</h3>
                <p className="note" style={{ marginBottom: 8 }}>
                  자동 생성: {q.contentPhraseAuto || '(일반 품목이 없어 만들 수 없습니다)'}
                </p>
                <div className="row">
                  {/* 확정될 때(초점 이동·Enter)만 저장한다 — 한글 조합을 깨뜨리지 않기 위해 */}
                  <div style={{ flex: 1 }}>
                    <TextField
                      key={`phrase-${q.id}`}
                      value={q.contentPhrase}
                      ariaLabel="품의 내용 문구"
                      style={{ width: '100%' }}
                      onCommit={(v) =>
                        act(() => ipc.setPhraseOverride(q.id, v === q.contentPhraseAuto ? null : v))
                      }
                    />
                  </div>
                  {q.contentPhraseOverride !== null && (
                    <button
                      onClick={() => act(() => ipc.setPhraseOverride(q.id, null))}
                    >
                      자동 문구로 되돌리기
                    </button>
                  )}
                </div>
                {q.contentPhraseOverride !== null && (
                  <p className="note">직접 고친 문구를 쓰고 있습니다.</p>
                )}
              </div>
            </div>
          )}
        </div>
      )}
    </>
  )
}
