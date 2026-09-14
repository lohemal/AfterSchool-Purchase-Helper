/**
 * 기다리는 일에 **끝나는 시각**을 준다.
 *
 * 화면이 "불러오는 중…" 에 갇히지 않게 하려는 것이다.
 * 시간이 넘으면 `TimeoutError` 로 끝내고, 부르는 쪽이 안내 문구를 보여 준다.
 *
 * 원래 하던 일을 **중간에 멈추지는 못한다**(Rust 쪽은 계속 돈다).
 * 다만 화면은 더 기다리지 않고 사람에게 다음 길을 알려 준다.
 */
export class TimeoutError extends Error {
  readonly ms: number
  constructor(ms: number) {
    super(`${Math.round(ms / 1000)}초 안에 끝나지 않았습니다.`)
    this.name = 'TimeoutError'
    this.ms = ms
  }
}

export function isTimeout(e: unknown): boolean {
  return e instanceof TimeoutError || (e as { name?: string })?.name === 'TimeoutError'
}

export function withTimeout<T>(work: Promise<T>, ms: number): Promise<T> {
  return new Promise<T>((resolve, reject) => {
    const timer = setTimeout(() => reject(new TimeoutError(ms)), ms)
    work.then(
      (v) => {
        clearTimeout(timer)
        resolve(v)
      },
      (e) => {
        clearTimeout(timer)
        reject(e)
      },
    )
  })
}
