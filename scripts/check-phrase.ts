// 문구 생성 규칙 시험. Rust 쪽 domain/phrase.rs 와 **같은 결과**가 나와야 한다.
// 실행: npm run check:model

import assert from 'node:assert/strict'
import { buildPhrase, defaultPhraseName, defaultRepresentative, itemCount } from '../src/lib/phrase.ts'
import type { PhraseRow } from '../src/lib/phrase.ts'

const row = (kind: PhraseRow['kind'], displayName: string): PhraseRow => ({ kind, displayName })

// --- 사람이 실제로 쓴 문구와 글자까지 같아야 한다 (02 문서 6-1) ---
assert.equal(
  buildPhrase('로봇과학부', [row('item', '프로보테크닉 교구'), row('item', '프로보테크닉 교재')]),
  '로봇과학부 프로보테크닉 교구 외 1종',
)
assert.equal(
  buildPhrase('주산암산부', [
    row('item', '방과후 기초Yap! 상'),
    row('item', '방과후 기초Yap! 하'),
    row('item', '10급Yap!'),
    row('item', '암산교재'),
  ]),
  '주산암산부 방과후 기초Yap! 상 외 3종',
)

// --- 품목명은 원문 그대로. 사람이 줄인 것을 따라 하지 않는다 (14장 2번) ---
assert.equal(
  buildPhrase('바둑부', [row('item', '바둑교재(상상바둑)')]),
  '바둑부 바둑교재(상상바둑) 1종',
)

// --- 사용자가 표시명을 고치면 그 값이 쓰인다 (14장 12번) ---
assert.equal(
  buildPhrase('항공드론부', [row('item', '드론항공과학 교구세트')]),
  '항공드론부 드론항공과학 교구세트 1종',
)

// --- 할인 행은 N종·대표품목에서 빠진다 (14장 9번) ---
const withDiscount = [row('item', '교재A'), row('item', '교재B'), row('adjustment', '할인')]
assert.equal(itemCount(withDiscount), 2)
assert.equal(buildPhrase('○○부', withDiscount), '○○부 교재A 외 1종')

// --- 금액 0원 행(사은품)도 마찬가지 (14장 10번) ---
const withGift = [row('item', '교재A'), row('item', '교재B'), row('zero', '사은품 노트')]
assert.equal(itemCount(withGift), 2)
assert.equal(buildPhrase('○○부', withGift), '○○부 교재A 외 1종')

// --- 할인이 앞에 있어도 대표품목은 첫 '일반 품목' ---
const adjustFirst = [row('adjustment', '할인'), row('zero', '사은품'), row('item', '교재A')]
assert.equal(defaultRepresentative(adjustFirst), 2)
assert.equal(buildPhrase('○○부', adjustFirst), '○○부 교재A 1종')

// --- 사용자가 대표품목을 고를 수 있다 ---
const three = [row('item', '교재A'), row('item', '교재B'), row('item', '교재C')]
assert.equal(buildPhrase('○○부', three, 1), '○○부 교재B 외 2종')
assert.equal(buildPhrase('○○부', three, 99), '○○부 교재A 외 2종', '엉뚱한 자리는 기본값으로')

// --- 일반 품목이 없으면 문구도 없다 ---
assert.equal(buildPhrase('○○부', [row('adjustment', '할인')]), null)

// --- 품의 표기명 기본값 ---
assert.equal(defaultPhraseName('바둑'), '바둑부')
assert.equal(defaultPhraseName('  토탈공예미니어처 '), '토탈공예미니어처부')

console.log('check-phrase: 통과')
