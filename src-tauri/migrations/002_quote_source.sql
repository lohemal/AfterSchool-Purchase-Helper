-- P2·P3: 견적서를 **어느 경로로** 읽었는지와 그 결과를 얼마나 믿을 수 있는지 남긴다.
--
--   source  structured = XLSX·XLS·XLSM·HWP·HWPX (값이 파일 안에 그대로 있다)
--           pdf_text   = PDF 안의 글자를 좌표로 되살림
--           ocr        = 사진·스캔 PDF 를 Windows 내장 글자 인식으로 읽음
--   trust   ok | needs_check | uncertain | amount_failed
--
-- P1 에서 읽어 둔 자료는 모두 structured 다 (그때는 사진·PDF 를 읽지 못했다).

ALTER TABLE work_quote ADD COLUMN source TEXT NOT NULL DEFAULT 'structured';
ALTER TABLE work_quote ADD COLUMN trust  TEXT NOT NULL DEFAULT '';
-- 사진에서 읽은 합계 원문 (`₩68,000` 이 `鬧8,000` 으로 읽히는 일이 있다. 고치지 않고 보여 주기만 한다)
ALTER TABLE work_quote ADD COLUMN raw_grand_total  TEXT NOT NULL DEFAULT '';
ALTER TABLE work_quote ADD COLUMN raw_supply_total TEXT NOT NULL DEFAULT '';
