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
  return execSync('git ls-files', { encoding: 'utf8' }).split('\n').filter(Boolean)
}

let blocked = 0
let review = 0
const files = trackedFiles()

for (const f of files) {
  // 이 검사기 자신은 찾을 낱말 목록을 담고 있으므로 건너뛴다
  if (f.endsWith('check-public-safe.mjs')) continue
  const lower = f.toLowerCase()
  const ext = lower.slice(lower.lastIndexOf('.'))

  if (BLOCK_EXT.includes(ext)) {
    console.error(`✗ 막음  ${f}\n        이 형식은 실제 업무 자료일 수 있어 공개 저장소에 두지 않는다 (${ext})`)
    blocked++
    continue
  }
  if (SKIP_READ.includes(ext)) continue
  try {
    if (statSync(f).size > 2_000_000) continue
  } catch {
    continue
  }

  let text
  try {
    text = readFileSync(f, 'utf8')
  } catch {
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
console.log(`검사한 파일 ${files.length}개 ${ALL ? '(폴더 전체)' : '(Git 추적 대상)'}`)
console.log(`  막음 ${blocked}건 · 살핌 ${review}건`)
if (blocked > 0) {
  console.error('\n공개하면 안 되는 것이 있습니다. 지우거나 가린 뒤 다시 검사하세요.')
  process.exit(1)
}
if (review > 0) {
  console.warn('\n막을 것은 없습니다. 「살핌」 항목이 공개해도 되는 자료인지 사람이 판단하세요.')
  process.exit(2)
}
console.log('\ncheck-public: 통과')
