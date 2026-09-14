-- 견적서 통합 관리 — 최초 스키마 (설계안 4장)
--
-- 용어를 절대 섞지 않는다 (설계안 2장):
--   department        품의 부서        — 품의 문구에 나오는 단위
--   settlement_alias  정산 별칭        — 정산자료 '부서명' 열의 원문. 거래처와 무관하다
--   vendor_unit       거래처 관리 단위  — 견적서 1장 = 품의 1행
-- 금액은 전부 원 단위 정수(INTEGER)다. 소수를 쓰지 않는다.

-- ---------------------------------------------------------------- 설정

CREATE TABLE department (
  id           INTEGER PRIMARY KEY AUTOINCREMENT,
  display_name TEXT    NOT NULL UNIQUE,          -- '토탈공예미니어처'
  phrase_name  TEXT    NOT NULL,                 -- '토탈공예미니어처부' (품의 문구 접두)
  sort_order   INTEGER NOT NULL DEFAULT 0,
  active       INTEGER NOT NULL DEFAULT 1 CHECK (active IN (0, 1))
);

-- 정산자료 '부서명' 열에 나타나는 이름. 한 부서에 여럿 붙을 수 있고,
-- 붙은 것들의 재원 금액은 자동으로 합산된다 (설계안 14장 4번).
-- 숫자 접미사를 프로그램이 해석하지 않는다. 파일명 매칭에도 쓰지 않는다.
CREATE TABLE settlement_alias (
  id            INTEGER PRIMARY KEY AUTOINCREMENT,
  department_id INTEGER NOT NULL REFERENCES department(id) ON DELETE CASCADE,
  alias         TEXT    NOT NULL UNIQUE           -- '토탈공예미니어처1'
);
CREATE INDEX idx_alias_dept ON settlement_alias(department_id);

-- 거래처 관리 단위. 견적서 파일명에 넣는 이름이 mgmt_name 이다.
CREATE TABLE vendor_unit (
  id            INTEGER PRIMARY KEY AUTOINCREMENT,
  department_id INTEGER NOT NULL REFERENCES department(id) ON DELETE CASCADE,
  mgmt_name     TEXT    NOT NULL UNIQUE,          -- '로봇과학1'
  vendor_name   TEXT,                             -- '(주)가나상사' (선택, 참고용)
  note          TEXT,
  sort_order    INTEGER NOT NULL DEFAULT 0,
  active        INTEGER NOT NULL DEFAULT 1 CHECK (active IN (0, 1))
);
CREATE INDEX idx_vendor_dept ON vendor_unit(department_id);

CREATE TABLE setting (
  key   TEXT PRIMARY KEY,
  value TEXT NOT NULL
);

-- ---------------------------------------------------------------- 작업

CREATE TABLE work (
  id          INTEGER PRIMARY KEY AUTOINCREMENT,
  title       TEXT    NOT NULL,
  school_year TEXT    NOT NULL DEFAULT '',        -- '2026학년도'
  month       TEXT    NOT NULL DEFAULT '',        -- '9월'
  kind        TEXT    NOT NULL DEFAULT '교재비',
  created_at  TEXT    NOT NULL,
  updated_at  TEXT    NOT NULL,
  status      TEXT    NOT NULL DEFAULT 'open'
);

-- 등록된 견적서 1장. 원본 파일은 복사하지 않고 경로·크기·해시만 둔다 (설계안 14장 15번).
CREATE TABLE work_quote (
  id             INTEGER PRIMARY KEY AUTOINCREMENT,
  work_id        INTEGER NOT NULL REFERENCES work(id) ON DELETE CASCADE,
  vendor_unit_id INTEGER REFERENCES vendor_unit(id) ON DELETE SET NULL,  -- NULL = 미매칭
  source_path    TEXT    NOT NULL,
  source_name    TEXT    NOT NULL,
  source_size    INTEGER NOT NULL DEFAULT 0,
  source_hash    TEXT    NOT NULL DEFAULT '',
  format         TEXT    NOT NULL,                -- xlsx|xls|xlsm|hwp|hwpx|pdf|image
  match_method   TEXT    NOT NULL DEFAULT 'none' CHECK (match_method IN ('auto','manual','none')),
  match_note     TEXT    NOT NULL DEFAULT '',     -- '로봇과학 부서는 거래처가 여러 개입니다' 등
  parse_status   TEXT    NOT NULL DEFAULT 'pending',
  parse_error    TEXT    NOT NULL DEFAULT '',
  parsed_at      TEXT,
  table_note     TEXT    NOT NULL DEFAULT '',     -- 표를 무엇으로 골랐는지
  supply_total   INTEGER,                         -- 견적서에 적힌 공급가액
  tax_total      INTEGER,                         -- 견적서에 적힌 세액
  grand_total    INTEGER,                         -- 견적서에 적힌 합계금액
  item_sum       INTEGER NOT NULL DEFAULT 0,      -- kind=item 합 (프로그램 계산)
  adjustment_sum INTEGER NOT NULL DEFAULT 0,      -- kind=adjustment 를 sign_effect 대로 적용한 합
  computed_total INTEGER NOT NULL DEFAULT 0,      -- item_sum + adjustment_sum
  compare_basis  TEXT    NOT NULL DEFAULT 'grand'
                 CHECK (compare_basis IN ('grand','supply','computed_total')),
  representative_item_id INTEGER,                 -- work_quote_item.id
  content_phrase_auto     TEXT NOT NULL DEFAULT '',
  content_phrase_override TEXT,
  vendor_name_in_doc TEXT NOT NULL DEFAULT '',    -- 견적서 안에서 읽은 상호 (참고용)
  warnings       TEXT    NOT NULL DEFAULT '[]',
  notes          TEXT    NOT NULL DEFAULT ''
);
CREATE INDEX idx_quote_work ON work_quote(work_id);
-- 같은 거래처 관리 단위에 견적서 둘이 붙으면 안 된다 (설계안 8-1)
CREATE UNIQUE INDEX idx_quote_vendor_once
  ON work_quote(work_id, vendor_unit_id) WHERE vendor_unit_id IS NOT NULL;

-- 추출 행. raw_* 는 견적서 원문이고 **절대 바뀌지 않는다** (설계안 14장 12번).
CREATE TABLE work_quote_item (
  id            INTEGER PRIMARY KEY AUTOINCREMENT,
  work_quote_id INTEGER NOT NULL REFERENCES work_quote(id) ON DELETE CASCADE,
  row_no        INTEGER NOT NULL,
  kind          TEXT    NOT NULL CHECK (kind IN ('item','adjustment','zero')),
  sign_effect   TEXT    CHECK (sign_effect IN ('as_written','subtract','unknown')),
  display_name  TEXT    NOT NULL,                 -- 품의 문구에 쓸 값. 기본값 = raw_name
  spec          TEXT    NOT NULL DEFAULT '',
  qty           INTEGER,
  unit_price    INTEGER,
  amount        INTEGER,                          -- 원문 부호 그대로
  raw_name       TEXT NOT NULL DEFAULT '',
  raw_spec       TEXT NOT NULL DEFAULT '',
  raw_qty        TEXT NOT NULL DEFAULT '',
  raw_unit_price TEXT NOT NULL DEFAULT '',
  raw_amount     TEXT NOT NULL DEFAULT '',
  edited        INTEGER NOT NULL DEFAULT 0 CHECK (edited IN (0, 1)),
  cell_ref      TEXT    NOT NULL DEFAULT '',      -- 원본 셀 위치 (예: 'B12' / 'T1 R10 C0')
  confidence    TEXT    NOT NULL DEFAULT 'high' CHECK (confidence IN ('high','medium','low')),
  warnings      TEXT    NOT NULL DEFAULT '[]'
);
CREATE INDEX idx_item_quote ON work_quote_item(work_quote_id);

-- 정산자료 한 줄. 원문 그대로이며 프로그램이 고치지 않는다.
CREATE TABLE work_settlement_row (
  id            INTEGER PRIMARY KEY AUTOINCREMENT,
  work_id       INTEGER NOT NULL REFERENCES work(id) ON DELETE CASCADE,
  row_no        INTEGER NOT NULL,
  source_name   TEXT    NOT NULL,                 -- 정산자료 원문 이름
  department_id INTEGER REFERENCES department(id) ON DELETE SET NULL,  -- NULL = 미등록 별칭
  beneficiary   INTEGER NOT NULL DEFAULT 0,
  excess        INTEGER NOT NULL DEFAULT 0,
  subsidy       INTEGER NOT NULL DEFAULT 0,
  voucher       INTEGER NOT NULL DEFAULT 0,
  stated_total  INTEGER
);
CREATE INDEX idx_settle_work ON work_settlement_row(work_id);

-- 거래처별 재원 배분 = 품의 한 행의 금액 (설계안 14장 5·6번).
-- 자동 비율 배분은 하지 않는다. source='auto' 는 '부서에 거래처가 하나여서 그대로 옮긴 것' 뿐이다.
CREATE TABLE work_allocation (
  id             INTEGER PRIMARY KEY AUTOINCREMENT,
  work_id        INTEGER NOT NULL REFERENCES work(id) ON DELETE CASCADE,
  vendor_unit_id INTEGER NOT NULL REFERENCES vendor_unit(id) ON DELETE CASCADE,
  beneficiary    INTEGER NOT NULL DEFAULT 0,
  excess         INTEGER NOT NULL DEFAULT 0,
  subsidy        INTEGER NOT NULL DEFAULT 0,
  voucher        INTEGER NOT NULL DEFAULT 0,
  source         TEXT    NOT NULL DEFAULT 'manual' CHECK (source IN ('auto','manual')),
  updated_at     TEXT    NOT NULL,
  UNIQUE (work_id, vendor_unit_id)
);

-- 검증 결과와 사용자 확인 (설계안 14장 13번).
CREATE TABLE work_check (
  id             INTEGER PRIMARY KEY AUTOINCREMENT,
  work_id        INTEGER NOT NULL REFERENCES work(id) ON DELETE CASCADE,
  kind           TEXT    NOT NULL,   -- vertical|horizontal|missing_quote|missing_settlement|unmapped_alias|...
  department_id  INTEGER,
  vendor_unit_id INTEGER,
  fund           TEXT,               -- 세로 검증일 때 어느 재원인지
  label          TEXT    NOT NULL DEFAULT '',
  expected       INTEGER,
  actual         INTEGER,
  diff           INTEGER,
  status         TEXT    NOT NULL CHECK (status IN ('ok','warn','error')),
  acknowledged   INTEGER NOT NULL DEFAULT 0 CHECK (acknowledged IN (0, 1)),
  ack_reason     TEXT    NOT NULL DEFAULT '',
  ack_at         TEXT
);
CREATE INDEX idx_check_work ON work_check(work_id);

CREATE TABLE work_output (
  id         INTEGER PRIMARY KEY AUTOINCREMENT,
  work_id    INTEGER NOT NULL REFERENCES work(id) ON DELETE CASCADE,
  fund       TEXT    NOT NULL CHECK (fund IN ('beneficiary','excess','subsidy','voucher')),
  path       TEXT    NOT NULL,
  row_count  INTEGER NOT NULL,
  total      INTEGER NOT NULL,
  created_at TEXT    NOT NULL,
  verified   INTEGER NOT NULL DEFAULT 0 CHECK (verified IN (0, 1))
);
CREATE INDEX idx_output_work ON work_output(work_id);
