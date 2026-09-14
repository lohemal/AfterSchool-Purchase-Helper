/**
 * 릴리스 산출물이 서로 맞는지 확인한다.
 *
 *   npm run check:release
 *
 * 앱이 들고 있는 공개 키로 설치 파일의 서명을 실제로 검증한다.
 * Tauri updater 가 하는 일과 같다 (minisign / ed25519).
 */
import { readFileSync } from 'node:fs'
import { createHash, verify } from 'node:crypto'

const conf = JSON.parse(readFileSync('src-tauri/tauri.conf.json', 'utf8'))
const pubB64 = conf.plugins.updater.pubkey
const pubTxt = Buffer.from(pubB64, 'base64').toString('utf8')
const pubLine = pubTxt.trim().split('\n').pop().trim()
const pubRaw = Buffer.from(pubLine, 'base64')       // [2]sig_alg [8]key_id [32]pubkey
const alg = pubRaw.subarray(0, 2).toString('latin1')
const keyId = pubRaw.subarray(2, 10)
const pubKey = pubRaw.subarray(10, 42)

const sigTxt = Buffer.from(readFileSync('release/afterschool-purchase-helper_0.1.0_x64-setup.exe.sig', 'utf8').trim(), 'base64').toString('utf8')
const sigLines = sigTxt.split('\n')
const sigRaw = Buffer.from(sigLines[1].trim(), 'base64')  // [2]alg [8]key_id [64]sig
const sigAlg = sigRaw.subarray(0, 2).toString('latin1')
const sigKeyId = sigRaw.subarray(2, 10)
const sigBytes = sigRaw.subarray(10, 74)

console.log('공개 키 알고리즘 :', JSON.stringify(alg), alg === 'Ed' ? '(ed25519, prehashed)' : '')
console.log('서명   알고리즘 :', JSON.stringify(sigAlg))
console.log('키 ID 일치      :', keyId.equals(sigKeyId) ? '예' : '아니오 ← 다른 키로 서명됐다')

const file = readFileSync('release/afterschool-purchase-helper_0.1.0_x64-setup.exe')
// 'Ed' = prehashed: BLAKE2b-512 로 먼저 해시한 뒤 서명한다
const { blake2b512 } = await import('node:crypto').then(m => ({ blake2b512: () => m.createHash('blake2b512') }))
const digest = blake2b512().update(file).digest()

const spki = Buffer.concat([
  Buffer.from('302a300506032b6570032100', 'hex'),  // ed25519 SPKI 머리
  pubKey,
])
const keyObj = await import('node:crypto').then(m =>
  m.createPublicKey({ key: spki, format: 'der', type: 'spki' }))

const ok = verify(null, digest, keyObj, sigBytes)
console.log('서명 검증       :', ok ? '통과 — 이 설치 파일은 앱이 든 공개 키와 짝이다' : '실패')

// 서명 안의 trusted comment 에 원본 파일 이름이 들어 있다 (참고)
const tc = sigLines.find(l => l.startsWith('trusted comment:'))
if (tc) console.log('서명이 기록한 원본:', tc.replace(/^trusted comment: *timestamp:\d+\s*file:/, ''))

process.exit(ok ? 0 : 1)
