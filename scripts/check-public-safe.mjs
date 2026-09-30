/**
 * 공개 저장소에 올려도 되는지 검사한다.
 *
 *   npm run check:public          # Git 이 실제로 추적하는 파일만
 *   npm run check:public -- --all # 폴더 전체 (Git 초기화 전)
 *
 * **`.gitignore` 만 믿지 않는다.** 실제로 올라갈 파일의 내용을 읽어
 * 개인정보·실제 업무 자료·비밀 정보가 들어 있는지 본다.
 *
 * 찾는 것을 두 등급으로 나눈다.
 *   막음(block) — 절대 공개하면 안 되는 것. 하나라도 있으면 실패한다.
 *   살핌(review) — 실제 업무 자료일 수 있어 사람이 판단할 것.
 */
import { execSync } from 'node:child_process'
import { readFileSync, statSync } from 'node:fs'
import { readdirSync } from 'node:fs'
import { join, relative } from 'node:path'

const ALL = process.argv.includes('--all')

/** 절대 올라가면 안 되는 것 */
const BLOCK = [
  { name: '사업자등록번호', re: /\b\d{3}-\d{2}-\d{5}\b/ },
  { name: '주민등록번호', re: /\b\d{6}-[1-4]\d{6}\b/ },
  { name: '전화번호', re: /\b0\d{1,2}-\d{3,4}-\d{4}\b/ },
  { name: '계좌번호로 보이는 숫자', re: /\b\d{3}-\d{4}-\d{4}-\d{2}\b/ },
  { name: '학교 이름', re: /[가-힣]{2,}(초등학교|중학교|고등학교)/ },
  { name: '개인 키', re: /BEGIN [A-Z ]*PRIVATE KEY|untrusted comment: minisign encrypted secret key/ },
  { name: 'GitHub 토큰', re: /\bgh[pousr]_[A-Za-z0-9]{16,}|\bgithub_pat_[A-Za-z0-9_]{20,}/ },
  { name: 'API 키처럼 보이는 값', re: /(api[_-]?key|secret|password)\s*[:=]\s*['"][^'"\s]{12,}['"]/i },
  { name: '개인 컴퓨터 경로', re: new RegExp('[A-Z]:' + String.fromCharCode(92) + '+Users' + String.fromCharCode(92) + '+(?!<)[A-Za-z0-9._-]+') },
]

/** 사람이 판단할 것 (실제 업무 자료일 수 있다) */
const REVIEW = [
  { name: '실제 거래처·상품 이름', re: /주산과암산|상상바둑|프로보테크닉/ },
  { name: '실제 대표자 이름으로 보이는 것', re: /성명 [가-힣]{2,4}[ ,)]|대표자 [가-힣]{2,4}[ ,)]/ },
  { name: '실제 주소', re: /(경남|경기|서울|부산|대구|인천|광주|대전|울산|세종|강원|충북|충남|전북|전남|경북|제주)\s?\S*(시|군|구)\s/ },
]

/** 이 확장자는 내용을 읽지 않고 **있다는 것만으로** 막는다 */
const BLOCK_EXT = ['.hwp', '.hwpx', '.xlsx', '.xls', '.xlsm', '.db', '.sqlite', '.key', '.pem', '.pfx', '.p12', '.env']
/** 내용을 읽지 않는 것 (그림·글꼴 등) */
const SKIP_READ = ['.png', '.ico', '.icns', '.jpg', '.jpeg', '.pdf', '.woff', '.woff2', '.ttf', '.zip']

function trackedFiles() {
  if (ALL) {
    const out = []
    const skipDir = new Set(['node_modules', 'target', 'dist', '.git', 'fixtures-local', 'out'])
    const walk = (dir) => {
      for (const e of readdirSync(dir, { withFileTypes: true })) {
        if (e.isDirectory()) {
          if (!skipDir.has(e.name)) walk(join(dir, e.name))
        } else out.push(relative('.', join(dir, e.name)).split(String.fromCharCode(92)).join('/'))
      }
    }
    walk('.')
    return out
  }
  // **`-z` 가 반드시 필요하다.** 그냥 `git ls-files` 는 한글처럼 ASCII 가 아닌 이름을
  // 따옴표에 넣고 8진 이스케이프해서 내놓는다(`core.quotepath` 기본값).
  // 그 문자열로 파일을 열면 없는 파일이라 그냥 넘어가 버려서 **검사하지 않은 채 통과**한다.
  // 실제로 이 저장소의 `docs/` 아홉 개가 통째로 빠져 있었다 — 공개 직전에 발견했다.
  return execSync('git ls-files -z', { encoding: 'utf8', maxBuffer: 32 * 1024 * 1024 })
    .split('\0')
    .filter(Boolean)
}

let blocked = 0
let review = 0
/** 실제로 내용을 읽은 파일 수 — 목록에 있다고 읽은 것이 아니다 */
let read = 0
/** 형식상 읽지 않기로 한 파일 수 (그림·글꼴 등) */
let skipped = 0
const files = trackedFiles()

for (const f of files) {
  // 이 검사기 자신은 찾을 낱말 목록을 담고 있으므로 건너뛴다
  if (f.endsWith('check-public-safe.mjs')) { skipped++; continue }
  const lower = f.toLowerCase()
  const ext = lower.slice(lower.lastIndexOf('.'))

  if (BLOCK_EXT.includes(ext)) {
    console.error(`✗ 막음  ${f}\n        이 형식은 실제 업무 자료일 수 있어 공개 저장소에 두지 않는다 (${ext})`)
    blocked++
    skipped++
    continue
  }
  if (SKIP_READ.includes(ext)) { skipped++; continue }
  // 읽지 못한 파일은 **검사하지 못한 파일**이다. 조용히 넘기면 통과한 것처럼 보인다.
  try {
    if (statSync(f).size > 2_000_000) { skipped++; continue }
  } catch (e) {
    console.error(`✗ 막음  ${f}\n        파일을 찾지 못해 검사하지 못했습니다 — ${e.code ?? e.message}`)
    blocked++
    continue
  }

  let text
  try {
    text = readFileSync(f, 'utf8')
    read++
  } catch (e) {
    console.error(`✗ 막음  ${f}\n        읽지 못해 검사하지 못했습니다 — ${e.code ?? e.message}`)
    blocked++
    continue
  }

  for (const { name, re } of BLOCK) {
    const m = text.match(re)
    if (m) {
      const line = text.slice(0, m.index).split('\n').length
      console.error(`✗ 막음  ${f}:${line}\n        ${name} — ${JSON.stringify(m[0].slice(0, 60))}`)
      blocked++
    }
  }
  for (const { name, re } of REVIEW) {
    const m = text.match(re)
    if (m) {
      const line = text.slice(0, m.index).split('\n').length
      console.warn(`● 살핌  ${f}:${line}\n        ${name} — ${JSON.stringify(m[0].slice(0, 60))}`)
      review++
    }
  }
}

console.log('')
// **읽은 개수를 따로 센다.** 목록에 있다고 읽은 것이 아니다.
// 한글 이름을 이스케이프한 경로로 열지 못해 `docs/` 아홉 개가 통째로 빠졌는데,
// 「검사한 파일 185개」만 찍고 있어서 드러나지 않았다.
console.log(`목록 ${files.length}개 ${ALL ? '(폴더 전체)' : '(Git 추적 대상)'}`)
console.log(`  내용을 읽은 파일 ${read}개 · 형식상 건너뛴 파일 ${skipped}개`)
console.log(`  막음 ${blocked}건 · 살핌 ${review}건`)
if (read + skipped !== files.length) {
  console.error(
    `\n✗ 목록(${files.length})과 읽음(${read})+건너뜀(${skipped})이 맞지 않습니다.` +
      '\n  검사하지 못한 파일이 있다는 뜻입니다.',
  )
  process.exit(1)
}
if (blocked > 0) {
  console.error('\n공개하면 안 되는 것이 있습니다. 지우거나 가린 뒤 다시 검사하세요.')
  process.exit(1)
}
if (review > 0) {
  console.warn('\n막을 것은 없습니다. 「살핌」 항목이 공개해도 되는 자료인지 사람이 판단하세요.')
  process.exit(2)
}
console.log('\ncheck-public: 통과')
