/**
 * 세 곳의 버전이 같은지 본다. 어긋나면 실패한다.
 *
 *   npm run check:version
 *
 * 버전이 어긋난 채로 배포하면 업데이터가 새 버전을 알아보지 못하거나
 * 설치 파일 이름과 latest.json 이 맞지 않아 업데이트가 조용히 깨진다.
 */
import { readFileSync } from 'node:fs'

const pkg = JSON.parse(readFileSync('package.json', 'utf8')).version
const conf = JSON.parse(readFileSync('src-tauri/tauri.conf.json', 'utf8')).version
const cargo = (readFileSync('src-tauri/Cargo.toml', 'utf8').match(/^version = "(.*)"$/m) || [])[1]

const all = { 'package.json': pkg, 'tauri.conf.json': conf, 'Cargo.toml': cargo }
for (const [where, v] of Object.entries(all)) console.log(`  ${where.padEnd(18)} ${v}`)

const values = [...new Set(Object.values(all))]
if (values.length !== 1 || !values[0]) {
  console.error('\n✗ 버전이 서로 다릅니다. `npm run version:set <버전>` 으로 맞춰 주세요.')
  process.exit(1)
}
console.log(`\ncheck-version: 통과 (v${values[0]})`)
