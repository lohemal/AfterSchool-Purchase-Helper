/**
 * 작업 이름 규칙 시험 (브라우저 없이 도는 순수 함수).
 *   npm run check:work
 *
 * 여기서 보는 것: **화면이 막는 이름과 서버가 막는 이름이 같은가.**
 * 서버 쪽 규칙은 `src-tauri/src/domain/work_manage_tests.rs` 가 따로 지킨다.
 * 둘이 어긋나면 화면은 통과시켰는데 저장이 안 되거나, 이름 없는 작업이 목록에 남는다.
 */
import {
  cleanWorkName,
  defaultWorkName,
  isValidWorkName,
  monthOf,
  schoolYearOf,
  workNameError,
} from '../src/lib/workName.ts'

let fail = 0
function eq(got: unknown, want: unknown, what: string) {
  if (JSON.stringify(got) !== JSON.stringify(want)) {
    console.error(`✗ ${what}\n  받음: ${JSON.stringify(got)}\n  기대: ${JSON.stringify(want)}`)
    fail++
  } else {
    console.log(`✓ ${what}`)
  }
}

// ---------------------------------------------------------------- 이름 다듬기

eq(cleanWorkName('2026학년도 9월 교재비'), '2026학년도 9월 교재비', '멀쩡한 이름은 그대로')
eq(cleanWorkName('  9월 교재비  '), '9월 교재비', '앞뒤 공백을 뗀다')
eq(cleanWorkName('9월  교재비'), '9월  교재비', '가운데 공백은 사용자가 친 그대로 둔다')
eq(cleanWorkName('\t9월 교재비\n'), '9월 교재비', '탭·줄바꿈도 뗀다')

// ---------------------------------------------------------------- 빈 이름 막기

eq(isValidWorkName('9월 교재비'), true, '이름이 있으면 쓸 수 있다')
for (const [bad, label] of [
  ['', '빈 문자열'],
  ['   ', '공백만'],
  ['\t', '탭만'],
  ['\n\r ', '줄바꿈만'],
  ['　', '전각 공백만 (한글 입력에서 잘 생긴다)'],
  ['　 　', '전각·반각 공백 섞인 것'],
] as const) {
  eq(isValidWorkName(bad), false, `${label} 은 막는다`)
  eq(workNameError(bad), '작업 이름을 입력해 주세요.', `${label} — 까닭을 알려 준다`)
}
eq(workNameError('9월 교재비'), null, '멀쩡한 이름에는 할 말이 없다')

// 이름이 공백뿐이면 다듬은 결과도 비어 있어야 한다 (서버로 보내기 전 마지막 확인)
eq(cleanWorkName('　  \t'), '', '공백만 있는 이름은 다듬으면 빈 문자열')

// ---------------------------------------------------------------- 미리 채워 두는 이름

// 3월부터 새 학년도
eq(schoolYearOf(new Date(2026, 2, 1)), '2026학년도', '3월 1일은 2026학년도')
eq(schoolYearOf(new Date(2026, 1, 28)), '2025학년도', '2월 28일은 아직 2025학년도')
eq(schoolYearOf(new Date(2026, 11, 31)), '2026학년도', '12월 31일은 2026학년도')
eq(schoolYearOf(new Date(2027, 0, 5)), '2026학년도', '1월 5일은 아직 2026학년도')

eq(monthOf(new Date(2026, 8, 30)), '9월', '9월')
eq(monthOf(new Date(2026, 0, 1)), '1월', '1월')

eq(defaultWorkName(new Date(2026, 8, 30)), '2026학년도 9월 교재비', '미리 채워 두는 이름')
eq(defaultWorkName(new Date(2026, 8, 30), '재료비'), '2026학년도 9월 재료비', '재료비 작업')
// 미리 채운 이름은 언제나 쓸 수 있어야 한다 (사용자가 그대로 눌러도 만들어져야 한다)
eq(isValidWorkName(defaultWorkName(new Date())), true, '미리 채운 이름은 언제나 유효하다')

console.log(fail === 0 ? '\ncheck-work: 통과' : `\ncheck-work: ${fail}건 실패`)
process.exit(fail === 0 ? 0 : 1)
