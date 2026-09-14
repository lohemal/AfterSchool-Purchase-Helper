/**
 * 배분 화면의 세로·가로 검증 (설계안 9장).
 * Rust 가 최종 판정을 하지만, 사용자가 숫자를 치는 동안 즉시 보여 주려면 같은 계산이 필요하다.
 * **자동 배분·추정은 여기에도 없다.**
 */

import type { DeptAllocationView, Fund, Funds } from '../ipc/types.ts'
import { FUNDS } from '../ipc/types.ts'

export const ZERO: Funds = { beneficiary: 0, excess: 0, subsidy: 0, voucher: 0 }

export function fundValue(f: Funds, key: Fund): number {
  return f[key]
}

export function withFund(f: Funds, key: Fund, value: number): Funds {
  return { ...f, [key]: value }
}

export function fundsTotal(f: Funds): number {
  return f.beneficiary + f.excess + f.subsidy + f.voucher
}

export interface VerticalCheck {
  fund: Fund
  sum: number
  expected: number
  diff: number
  ok: boolean
}

/** 세로 — 거래처별 같은 재원의 합 == 정산자료의 그 부서 재원 금액 */
export function verticalChecks(
  dept: DeptAllocationView,
  edited: Record<number, Funds>,
): VerticalCheck[] {
  return FUNDS.map((fund) => {
    const sum = dept.vendors.reduce(
      (acc, v) => acc + fundValue(edited[v.vendorUnitId] ?? v.allocated, fund),
      0,
    )
    const expected = fundValue(dept.settlement, fund)
    return { fund, sum, expected, diff: sum - expected, ok: sum === expected }
  })
}

export interface HorizontalCheck {
  vendorUnitId: number
  sum: number
  expected: number | null
  diff: number | null
  ok: boolean
}

/** 가로 — 한 거래처의 재원 4개 합 == 그 거래처 견적서 총액 */
export function horizontalChecks(
  dept: DeptAllocationView,
  edited: Record<number, Funds>,
): HorizontalCheck[] {
  return dept.vendors.map((v) => {
    const sum = fundsTotal(edited[v.vendorUnitId] ?? v.allocated)
    const expected = v.quoteTotal
    if (expected === null) {
      return { vendorUnitId: v.vendorUnitId, sum, expected: null, diff: null, ok: true }
    }
    return {
      vendorUnitId: v.vendorUnitId,
      sum,
      expected,
      diff: sum - expected,
      ok: sum === expected,
    }
  })
}

/** 이 부서가 통째로 맞는가 */
export function deptIsOk(dept: DeptAllocationView, edited: Record<number, Funds>): boolean {
  if (!dept.hasSettlement) return dept.vendors.every((v) => v.quoteTotal === null)
  return (
    verticalChecks(dept, edited).every((c) => c.ok) &&
    horizontalChecks(dept, edited).every((c) => c.ok)
  )
}
