/**
 * 파일명 매칭 **미리보기** (설계안 8-1).
 * 실제 판정은 Rust 가 한다. 화면에서 "이 이름이면 어디에 붙는지" 를 먼저 보여 주는 용도다.
 *
 * **정산 별칭은 여기에 들어오지 않는다.** 후보는 거래처 관리명과 품의 부서명뿐이다.
 */

export interface MatchCandidate {
  vendorUnitId: number
  mgmtName: string
  departmentId: number
  departmentName: string
}

export interface MatchPreview {
  vendorUnitId: number | null
  method: 'auto' | 'none'
  candidates: number[]
  note: string
}

/** 확장자 제거 → 공백·기호 제거 → 전각→반각 → 소문자 */
export function normalize(name: string): string {
  const dot = name.lastIndexOf('.')
  const stem = dot > 0 ? name.slice(0, dot) : name
  return [...stem]
    .map((c) => {
      const code = c.codePointAt(0)!
      if (code >= 0xff01 && code <= 0xff5e) return String.fromCodePoint(code - 0xfee0)
      if (c === '　') return ' '
      return c
    })
    .filter((c) => !' \t_-()[]{}.,'.includes(c))
    .join('')
    .toLowerCase()
}

export function matchFile(fileName: string, candidates: MatchCandidate[]): MatchPreview {
  const hay = normalize(fileName)

  // 1) 거래처 관리명 — 긴 것부터
  const byMgmt = candidates
    .filter((c) => {
      const n = normalize(c.mgmtName)
      return n !== '' && hay.includes(n)
    })
    .sort((a, b) => normalize(b.mgmtName).length - normalize(a.mgmtName).length)

  if (byMgmt.length > 0) {
    const bestLen = normalize(byMgmt[0].mgmtName).length
    const tied = byMgmt.filter((c) => normalize(c.mgmtName).length === bestLen)
    if (tied.length === 1) {
      return {
        vendorUnitId: tied[0].vendorUnitId,
        method: 'auto',
        candidates: [tied[0].vendorUnitId],
        note: `파일명에 거래처 관리명 '${tied[0].mgmtName}' 이 있습니다.`,
      }
    }
    return {
      vendorUnitId: null,
      method: 'none',
      candidates: tied.map((c) => c.vendorUnitId),
      note: '파일명에 맞는 거래처 관리명이 여러 개입니다. 직접 골라 주세요.',
    }
  }

  // 2) 품의 부서명 — 긴 것부터
  const deptHits: { id: number; name: string }[] = []
  for (const c of candidates) {
    const n = normalize(c.departmentName)
    if (n !== '' && hay.includes(n) && !deptHits.some((d) => d.id === c.departmentId)) {
      deptHits.push({ id: c.departmentId, name: c.departmentName })
    }
  }
  deptHits.sort((a, b) => normalize(b.name).length - normalize(a.name).length)

  if (deptHits.length > 0) {
    const longest = normalize(deptHits[0].name).length
    const tied = deptHits.filter((d) => normalize(d.name).length === longest)
    if (tied.length > 1) {
      return {
        vendorUnitId: null,
        method: 'none',
        candidates: candidates
          .filter((c) => tied.some((d) => d.id === c.departmentId))
          .map((c) => c.vendorUnitId),
        note: '파일명에 맞는 부서가 여러 개입니다. 직접 골라 주세요.',
      }
    }
    const inDept = candidates.filter((c) => c.departmentId === tied[0].id)
    // 3) 거래처가 하나면 자동
    if (inDept.length === 1) {
      return {
        vendorUnitId: inDept[0].vendorUnitId,
        method: 'auto',
        candidates: [inDept[0].vendorUnitId],
        note: `'${tied[0].name}' 부서에 거래처가 하나뿐이라 자동으로 정했습니다.`,
      }
    }
    // 4) 둘 이상이면 추측하지 않는다
    return {
      vendorUnitId: null,
      method: 'none',
      candidates: inDept.map((c) => c.vendorUnitId),
      note: `${tied[0].name} 부서는 거래처가 여러 개입니다. 거래처를 골라 주세요.`,
    }
  }

  // 5) 못 찾음
  return {
    vendorUnitId: null,
    method: 'none',
    candidates: [],
    note: '파일명에서 등록된 부서명이나 거래처 관리명을 찾지 못했습니다. 직접 골라 주세요.',
  }
}
