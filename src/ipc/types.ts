// Rust 쪽 구조와 1:1. 이름은 설계안 2장의 용어를 그대로 쓴다.

export type Fund = 'beneficiary' | 'excess' | 'subsidy' | 'voucher'

export const FUNDS: Fund[] = ['beneficiary', 'excess', 'subsidy', 'voucher']

export const FUND_LABEL: Record<Fund, string> = {
  beneficiary: '수익자',
  excess: '초과금',
  subsidy: '지원금',
  voucher: '자유수강권',
}

export interface Funds {
  beneficiary: number
  excess: number
  subsidy: number
  voucher: number
}

export const ZERO_FUNDS: Funds = { beneficiary: 0, excess: 0, subsidy: 0, voucher: 0 }

/** 견적서 행의 종류 (설계안 6-2) */
export type RowKind = 'item' | 'adjustment' | 'zero'

export const ROW_KIND_LABEL: Record<RowKind, string> = {
  item: '일반 품목',
  adjustment: '할인·조정',
  zero: '금액 0원',
}

/** 할인 금액이 총액에 들어가는 부호. 원문 값은 이것과 무관하게 그대로 남는다. */
export type SignEffect = 'asWritten' | 'subtract' | 'unknown'

export const SIGN_LABEL: Record<SignEffect, string> = {
  asWritten: '원문 그대로 더함',
  subtract: '빼는 값',
  unknown: '아직 정하지 않음',
}

export type CompareBasis = 'grand' | 'supply' | 'computedTotal'

export type Severity = 'info' | 'warn' | 'error'

export interface Warning {
  code: string
  message: string
  severity: Severity
}

/** 거래처 관리 단위 — 견적서 1장 = 품의 1행 */
export interface VendorUnit {
  id: number
  mgmtName: string
  vendorName: string
  note: string
  sortOrder: number
  active: boolean
}

/** 품의 부서 */
export interface Department {
  id: number
  displayName: string
  phraseName: string
  sortOrder: number
  active: boolean
  /** 정산자료 '부서명' 열에 나오는 이름들. 거래처와 무관하다. */
  aliases: string[]
  vendors: VendorUnit[]
}

export interface SetupExport {
  format: string
  exportedAt: string
  departments: Department[]
}

export interface Work {
  id: number
  title: string
  schoolYear: string
  month: string
  kind: string
  createdAt: string
  updatedAt: string
  status: string
}

export interface ItemRow {
  id: number
  rowNo: number
  kind: RowKind
  signEffect: SignEffect | null
  displayName: string
  spec: string
  qty: number | null
  unitPrice: number | null
  amount: number | null
  rawName: string
  rawSpec: string
  rawQty: string
  rawUnitPrice: string
  rawAmount: string
  edited: boolean
  cellRef: string
  confidence: 'high' | 'medium' | 'low'
  warnings: Warning[]
}

/** 원본 견적서의 한 쪽을 화면에서 보기 위한 그림 (P4-2) */
export interface PreviewPage {
  /** `사진` · `1쪽` — 품목의 `원본 위치` 와 같은 말 */
  label: string
  mime: string
  base64: string
  width: number
  height: number
}

export interface QuotePreview {
  pages: PreviewPage[]
  /** 그림을 만들 수 없을 때 사람에게 할 말 */
  note: string
}

/** 견적서를 읽어 온 경로 */
export type QuoteSource = 'structured' | 'pdf_text' | 'ocr'

/** 읽은 결과를 얼마나 믿을 수 있는가 */
export type QuoteTrust = '' | 'ok' | 'needs_check' | 'uncertain' | 'amount_failed'

/** 견적서를 읽는 동안 오는 진행 상황 (사진·PDF 는 오래 걸린다) */
export interface QuoteProgress {
  workId: number
  index: number
  total: number
  name: string
  slow: boolean
  stage: 'start' | 'done' | 'failed'
  message: string
}

export interface QuoteRow {
  id: number
  vendorUnitId: number | null
  vendorMgmtName: string
  departmentName: string
  phraseName: string
  sourcePath: string
  sourceName: string
  sourceExists: boolean
  format: string
  matchMethod: 'auto' | 'manual' | 'none'
  matchNote: string
  parseStatus: string
  parseError: string
  tableNote: string
  supplyTotal: number | null
  taxTotal: number | null
  grandTotal: number | null
  itemSum: number
  adjustmentSum: number
  computedTotal: number
  compareBasis: CompareBasis
  compareTotal: number
  /** 어느 경로로 읽었는가 */
  source: QuoteSource
  /** 한 줄 신뢰 상태 */
  trust: QuoteTrust
  /** 사진에서 읽은 합계 원문 (고치지 않고 보여 주기만 한다) */
  rawGrandTotal: string
  rawSupplyTotal: string
  representativeItemId: number | null
  contentPhrase: string
  contentPhraseAuto: string
  contentPhraseOverride: string | null
  vendorNameInDoc: string
  warnings: Warning[]
  items: ItemRow[]
}

export interface SettlementRowView {
  id: number
  rowNo: number
  /** 정산자료 원문 이름 */
  sourceName: string
  departmentId: number | null
  departmentName: string
  funds: Funds
  statedTotal: number | null
}

export interface VendorAllocationView {
  vendorUnitId: number
  mgmtName: string
  allocated: Funds
  missing: boolean
  quoteTotal: number | null
  /** 거래처가 하나여서 값을 고칠 수 없는가 */
  locked: boolean
}

export interface DeptAllocationView {
  departmentId: number
  departmentName: string
  settlement: Funds
  hasSettlement: boolean
  vendors: VendorAllocationView[]
}

export interface CheckRow {
  id: number
  kind: string
  label: string
  status: 'ok' | 'warn' | 'error'
  expected: number | null
  actual: number | null
  diff: number | null
  acknowledged: boolean
  ackReason: string
}

export interface Blocker {
  checkId: number
  kind: string
  label: string
  acknowledged: boolean
  ackReason: string
}

export interface GateResult {
  canGenerate: boolean
  openBlockers: Blocker[]
  acknowledged: Blocker[]
  message: string
}

export interface PumuiRow {
  vendorUnitId: number
  content: string
  amount: number
}

export interface FundPreview {
  fund: Fund
  fundLabel: string
  rows: PumuiRow[]
  total: number
  emptyNote: string
}

export interface CreatedFile {
  fundLabel: string
  path: string
  rowCount: number
  total: number
}

export interface GenerateResult {
  created: CreatedFile[]
  skipped: string[]
}

export interface AppError {
  code: string
  message: string
  detail: string | null
}
