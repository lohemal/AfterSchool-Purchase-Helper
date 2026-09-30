/**
 * Tauri 업데이터로 가는 유일한 통로.
 *
 * 내려받기와 **서명 확인**은 전부 Tauri 업데이터가 한다. 앱에 박혀 있는 공개 키로
 * `latest.json` 의 서명을 맞춰 보고, 맞지 않으면 설치하지 않는다.
 * 여기서 따로 내려받거나 그 확인을 건너뛰는 길은 만들지 않는다.
 *
 * 플러그인은 **쓸 때 불러온다**(동적 import). 시작이 느려지지 않고,
 * 업데이트와 상관없는 화면은 이 코드를 아예 건드리지 않는다.
 */
import type { CheckFn } from './updateCheck'

/** 지금 돌고 있는 프로그램의 버전 (`tauri.conf.json` 에 적힌 값) */
export async function currentVersion(): Promise<string> {
  const { getVersion } = await import('@tauri-apps/api/app')
  return await getVersion()
}

/** 새 버전을 찾아본다. 없으면 `null` */
export const checkForUpdate: CheckFn = async () => {
  const { check } = await import('@tauri-apps/plugin-updater')
  const found = await check()
  if (!found) return null
  return {
    version: found.version,
    install: async () => {
      await found.downloadAndInstall()
    },
  }
}
