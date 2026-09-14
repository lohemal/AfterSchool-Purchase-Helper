import { invoke } from '@tauri-apps/api/core'
import type {
  CheckRow,
  CompareBasis,
  Department,
  DeptAllocationView,
  Funds,
  FundPreview,
  GateResult,
  GenerateResult,
  QuotePreview,
  QuoteProgress,
  QuoteRow,
  SettlementRowView,
  SetupExport,
  Warning,
  Work,
} from './types'

/** Rust 의 AppError 를 사람이 읽는 문장으로 */
export function errorMessage(e: unknown): string {
  if (e && typeof e === 'object' && 'message' in e) return String((e as { message: unknown }).message)
  return String(e)
}

export function errorCode(e: unknown): string {
  if (e && typeof e === 'object' && 'code' in e) return String((e as { code: unknown }).code)
  return ''
}

// --- 부서 설정 ---
export const listDepartments = () => invoke<Department[]>('list_departments')
export const saveDepartment = (dept: Department) => invoke<number>('save_department', { dept })
export const deleteDepartment = (id: number) => invoke<void>('delete_department', { id })
export const exportSetup = () => invoke<SetupExport>('export_setup')
export const importSetup = (data: SetupExport) => invoke<number>('import_setup', { data })

// --- 작업 ---
export const listWorks = () => invoke<Work[]>('list_works')
export const getWork = (id: number) => invoke<Work>('get_work', { id })
export const createWork = (title: string, schoolYear: string, month: string, kind: string) =>
  invoke<number>('create_work', { title, schoolYear, month, kind })
export const deleteWork = (id: number) => invoke<void>('delete_work', { id })

// --- 견적서 ---
export const registerQuotes = (workId: number, paths: string[]) =>
  invoke<string[]>('register_quotes', { workId, paths })
export const listQuotes = (workId: number) => invoke<QuoteRow[]>('list_quotes', { workId })
export const setQuoteVendor = (quoteId: number, vendorUnitId: number | null) =>
  invoke<void>('set_quote_vendor', { quoteId, vendorUnitId })
export const deleteQuote = (quoteId: number) => invoke<void>('delete_quote', { quoteId })
export const reparseQuote = (quoteId: number) => invoke<void>('reparse_quote', { quoteId })

export const updateItem = (edit: {
  id: number
  kind: string
  signEffect: string | null
  displayName: string
  spec: string
  qty: number | null
  unitPrice: number | null
  amount: number | null
}) => invoke<void>('update_item', { edit })
export const addItem = (quoteId: number) => invoke<number>('add_item', { quoteId })
export const deleteItem = (itemId: number) => invoke<void>('delete_item', { itemId })
export const setRepresentative = (quoteId: number, itemId: number | null) =>
  invoke<void>('set_representative', { quoteId, itemId })
export const setPhraseOverride = (quoteId: number, text: string | null) =>
  invoke<void>('set_phrase_override', { quoteId, text })
export const setCompareBasis = (quoteId: number, basis: CompareBasis) =>
  invoke<void>('set_compare_basis', { quoteId, basis })

// --- 정산자료 ---
export const loadSettlement = (workId: number, path: string) =>
  invoke<Warning[]>('load_settlement', { workId, path })
export const listSettlement = (workId: number) =>
  invoke<SettlementRowView[]>('list_settlement', { workId })
export const mapAlias = (workId: number, sourceName: string, departmentId: number) =>
  invoke<void>('map_alias', { workId, sourceName, departmentId })

// --- 배분·검증 ---
export const listAllocations = (workId: number) =>
  invoke<DeptAllocationView[]>('list_allocations', { workId })
export const saveAllocation = (workId: number, vendorUnitId: number, funds: Funds) =>
  invoke<void>('save_allocation', { workId, vendorUnitId, funds })
export const runChecks = (workId: number) => invoke<CheckRow[]>('run_checks', { workId })
export const acknowledgeCheck = (checkId: number, reason: string) =>
  invoke<void>('acknowledge_check', { checkId, reason })
export const unacknowledgeCheck = (checkId: number) =>
  invoke<void>('unacknowledge_check', { checkId })
export const generationGate = (workId: number) => invoke<GateResult>('generation_gate', { workId })

// --- 미리보기·생성 ---
export const preview = (workId: number) => invoke<FundPreview[]>('preview', { workId })
export const generate = (workId: number, destDir: string) =>
  invoke<GenerateResult>('generate', { workId, destDir })
export const lastOutputDir = () => invoke<string | null>('last_output_dir')
export const hangulAvailable = () => invoke<boolean>('hangul_available')

/** 사진·스캔 PDF 를 읽을 수 있는 컴퓨터인가 */
export const ocrAvailable = () => invoke<boolean>('ocr_available')

/** 원본 견적서를 화면에서 보기 위한 그림 (사진·스캔 PDF 만 나온다) */
export const quotePreview = (quoteId: number) =>
  invoke<QuotePreview>('quote_preview', { quoteId })

/** 원본 파일을 이 컴퓨터의 기본 프로그램으로 연다 (엑셀·한글·PDF 뷰어) */
export async function openSourceFile(path: string): Promise<void> {
  const { openPath } = await import('@tauri-apps/plugin-opener')
  await openPath(path)
}

/** 견적서를 읽는 동안 오는 진행 상황을 듣는다. 되돌려주는 함수를 부르면 그만 듣는다. */
export async function onQuoteProgress(fn: (p: QuoteProgress) => void): Promise<() => void> {
  const { listen } = await import('@tauri-apps/api/event')
  const un = await listen<QuoteProgress>('quote-progress', (e) => fn(e.payload))
  return un
}
