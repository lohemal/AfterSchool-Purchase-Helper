// 파일명 매칭 시험. Rust 쪽 domain/matching.rs 와 **같은 판정**이어야 한다.
// 설계안 14장 1번의 규칙을 그대로 확인한다.

import assert from 'node:assert/strict'
import { matchFile, normalize } from '../src/lib/matchPreview.ts'
import type { MatchCandidate } from '../src/lib/matchPreview.ts'

const cand = (id: number, mgmt: string, deptId: number, dept: string): MatchCandidate => ({
  vendorUnitId: id,
  mgmtName: mgmt,
  departmentId: deptId,
  departmentName: dept,
})

// --- 파일명 형식을 강제하지 않는다. 아래는 모두 같게 매칭된다 ---
const single = [cand(1, '주산암산', 10, '주산암산')]
for (const name of [
  '주산암산부 견적서.jpg',
  '주산암산 견적서.jpg',
  '주산암산 견적서류.jpg',
  '주산암산 9월 견적서.jpg',
  '2026 주산암산_견적(최종).xlsx',
]) {
  const r = matchFile(name, single)
  assert.equal(r.vendorUnitId, 1, name)
  assert.equal(r.method, 'auto', name)
}

// --- 거래처가 여럿이면 관리명으로 갈린다. 긴 이름이 이긴다 ---
const two = [cand(1, '로봇과학1', 10, '로봇과학'), cand(2, '로봇과학2', 10, '로봇과학')]
assert.equal(matchFile('로봇과학1 견적서.hwp', two).vendorUnitId, 1)
assert.equal(matchFile('로봇과학2 견적서.xlsx', two).vendorUnitId, 2)

// --- 부서명만 있으면 **추측하지 않는다** ---
const guess = matchFile('로봇과학 견적서.hwp', two)
assert.equal(guess.vendorUnitId, null)
assert.equal(guess.method, 'none')
assert.deepEqual(guess.candidates, [1, 2])
assert.ok(guess.note.includes('거래처가 여러 개'), guess.note)

// --- 관리명이 부서명과 달라도 부서명으로 찾아 자동 매칭된다 ---
assert.equal(matchFile('바둑 견적서.xlsx', [cand(7, '바둑거래처A', 20, '바둑')]).vendorUnitId, 7)

// --- 못 찾으면 후보가 비어 있다 ---
const none = matchFile('무슨무슨 견적서.xlsx', [cand(1, '바둑', 20, '바둑')])
assert.equal(none.vendorUnitId, null)
assert.deepEqual(none.candidates, [])

// --- 정산 별칭은 후보가 아니므로 맞을 수가 없다. 숫자도 해석하지 않는다 ---
const craft = [cand(5, '토탈공예', 30, '토탈공예미니어처')]
const r = matchFile('토탈공예미니어처1 견적서.xlsx', craft)
assert.equal(r.vendorUnitId, 5, '부서명으로 찾아 거래처 하나에 붙는다')
assert.equal(r.method, 'auto')

// --- 더 긴 부서명이 이긴다 ---
const sci = [cand(1, '과학거래처', 10, '과학'), cand(2, '통합과학거래처', 20, '통합과학')]
assert.equal(matchFile('통합과학 견적서.xlsx', sci).vendorUnitId, 2)

// --- 정규화 ---
assert.equal(normalize('주산암산 견적서.jpg'), '주산암산견적서')
assert.equal(normalize('로봇과학1_견적서(최종).hwp'), '로봇과학1견적서최종')
assert.equal(normalize('ABC-123.xlsx'), 'abc123')
assert.equal(normalize('ＡＢＣ１２３.xlsx'), 'abc123')
assert.equal(normalize('바둑'), '바둑')

console.log('check-match: 통과')
