// 배분 화면의 세로·가로 검증 시험. Rust 쪽 domain/allocation.rs 와 같은 결과여야 한다.

import assert from 'node:assert/strict'
import {
  deptIsOk,
  fundsTotal,
  horizontalChecks,
  verticalChecks,
  withFund,
  ZERO,
} from '../src/lib/allocation.ts'
import { comma, diffText, parseMoney } from '../src/lib/money.ts'
import type { DeptAllocationView, Funds } from '../src/ipc/types.ts'

const f = (b: number, e: number, s: number, v: number): Funds => ({
  beneficiary: b,
  excess: e,
  subsidy: s,
  voucher: v,
})

// --- 바둑: 거래처 하나. 정산 금액이 그대로 옮겨져 세로·가로가 다 맞는다 ---
const baduk: DeptAllocationView = {
  departmentId: 1,
  departmentName: '바둑',
  settlement: f(372_000, 12_000, 60_000, 0),
  hasSettlement: true,
  vendors: [
    {
      vendorUnitId: 10,
      mgmtName: '바둑',
      allocated: f(372_000, 12_000, 60_000, 0),
      missing: false,
      quoteTotal: 444_000,
      locked: true,
    },
  ],
}
assert.ok(verticalChecks(baduk, {}).every((c) => c.ok), '바둑 세로')
assert.ok(horizontalChecks(baduk, {}).every((c) => c.ok), '바둑 가로')
assert.ok(deptIsOk(baduk, {}))

// --- 로봇과학: 거래처 둘. 사용자가 넣은 값으로 두 방향을 본다 ---
const robot: DeptAllocationView = {
  departmentId: 2,
  departmentName: '로봇과학',
  settlement: f(810_000, 6_200, 443_800, 0),
  hasSettlement: true,
  vendors: [
    {
      vendorUnitId: 20,
      mgmtName: '로봇과학1',
      allocated: f(720_000, 6_200, 173_800, 0),
      missing: false,
      quoteTotal: 900_000,
      locked: false,
    },
    {
      vendorUnitId: 21,
      mgmtName: '로봇과학2',
      allocated: f(90_000, 0, 270_000, 0),
      missing: false,
      quoteTotal: 360_000,
      locked: false,
    },
  ],
}
assert.ok(deptIsOk(robot, {}), '로봇과학은 세로·가로가 모두 맞아야 한다')

// 수익자만 1,000 모자라게 고쳐 본다 → 세로 하나만 어긋난다
const edited = { 21: withFund(robot.vendors[1].allocated, 'beneficiary', 89_000) }
const vs = verticalChecks(robot, edited)
const bad = vs.filter((c) => !c.ok)
assert.equal(bad.length, 1)
assert.equal(bad[0].fund, 'beneficiary')
assert.equal(bad[0].diff, -1_000)

// 가로도 함께 어긋난다 (재원 합이 줄었으므로)
const hs = horizontalChecks(robot, edited)
assert.equal(hs.filter((c) => !c.ok).length, 1)
assert.equal(hs.find((c) => c.vendorUnitId === 21)!.diff, -1_000)
assert.ok(!deptIsOk(robot, edited))

// --- 견적서가 없는 거래처는 가로 검증을 하지 않는다 ---
const noQuote: DeptAllocationView = {
  ...baduk,
  vendors: [{ ...baduk.vendors[0], quoteTotal: null }],
}
assert.ok(horizontalChecks(noQuote, {}).every((c) => c.ok))

// --- 금액 표시·입력 ---
assert.equal(comma(0), '0')
assert.equal(comma(444_000), '444,000')
assert.equal(comma(-30_000), '-30,000')
assert.equal(comma(null), '')

assert.equal(parseMoney('444,000'), 444_000)
assert.equal(parseMoney('  76,500원 '), 76_500)
assert.equal(parseMoney('-30,000'), -30_000)
assert.equal(parseMoney('(30,000)'), -30_000, '괄호 안이 숫자뿐이면 음수')
assert.equal(parseMoney('(￦ 900,000 )'), 900_000, '통화 기호가 있으면 음수가 아니다')
assert.equal(parseMoney(''), null)
assert.equal(parseMoney('없음'), null)

assert.equal(diffText(0), '일치')
assert.equal(diffText(10_000), '10,000원 많음')
assert.equal(diffText(-10_000), '10,000원 모자람')

assert.equal(fundsTotal(ZERO), 0)
assert.equal(fundsTotal(f(1, 2, 3, 4)), 10)

console.log('check-allocation: 통과')
