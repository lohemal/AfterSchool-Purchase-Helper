//! P1-12 실제 샘플 End-to-End 시험.
//!
//! 부서 설정 → 견적서 등록·매칭 → 추출 → 정산자료 → 배분·검증 → 미리보기 → 생성까지
//! 실제 업무 파일로 한 번에 돌린다. 샘플이 없으면 건너뛴다.
//!
//! HWP 가 필요한 시험은 `#[ignore]`:
//!   cargo test e2e_with_hwp -- --ignored --nocapture --test-threads=1

use super::*;
use crate::db::memory_db;
use crate::domain::setup::{Department, VendorUnit};
use crate::domain::Fund;

/// 실제 업무 샘플 폴더. **공개 저장소에는 없다**(개인정보·학교 회계 자료).
/// 없으면 이 시험들은 조용히 건너뛰고, 나머지 시험은 그대로 돈다.
fn fixtures() -> Option<std::path::PathBuf> {
    let p = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()?
        .join("test")
        .join("fixtures-local");
    if !p.is_dir() {
        eprintln!(
            "건너뜀 — 이 시험에는 로컬 전용 샘플 폴더가 필요합니다: test/fixtures-local/\n\
             (실제 업무 자료라 저장소에 넣지 않습니다. 자세한 내용은 docs/08-P5-배포준비.md)"
        );
        return None;
    }
    Some(p)
}

fn dept(name: &str, aliases: &[&str], vendors: &[&str]) -> Department {
    Department {
        id: 0,
        display_name: name.into(),
        phrase_name: format!("{name}부"),
        sort_order: 0,
        active: true,
        aliases: aliases.iter().map(|s| s.to_string()).collect(),
        vendors: vendors
            .iter()
            .enumerate()
            .map(|(i, v)| VendorUnit {
                id: 0,
                mgmt_name: v.to_string(),
                vendor_name: String::new(),
                note: String::new(),
                sort_order: i as i64,
                active: true,
            })
            .collect(),
    }
}

/// 실제 정산자료의 20개 부서를 그대로 등록한다.
/// 별칭 여럿 → 한 부서로 모이는 두 경우(토탈공예미니어처·키즈쿠킹)를 포함한다.
fn setup_real_departments(db: &crate::db::Db) {
    db.write(|c| {
        for (name, aliases, vendors) in [
            ("로봇과학", &["로봇과학"][..], &["로봇과학"][..]),
            ("만화애니메이션", &["만화애니메이션"], &["만화애니메이션"]),
            ("키즈쿠킹", &["목요키즈쿠킹", "화요키즈쿠킹"], &["키즈쿠킹"]),
            ("바둑", &["바둑"], &["바둑"]),
            ("역사탐구", &["역사탐구"], &["역사탐구"]),
            ("영어회화", &["영어회화"], &["영어회화"]),
            ("우쿠렐레", &["우쿠렐레"], &["우쿠렐레"]),
            ("종이접기", &["종이접기"], &["종이접기"]),
            ("주산암산", &["주산암산"], &["주산암산"]),
            ("창의독서", &["창의독서"], &["창의독서"]),
            ("체스", &["체스"], &["체스"]),
            ("컴퓨터", &["컴퓨터"], &["컴퓨터"]),
            ("코딩", &["코딩"], &["코딩"]),
            (
                "토탈공예미니어처",
                &["토탈공예미니어처", "토탈공예미니어처1", "토탈공예미니어처2"],
                &["토탈공예미니어처"],
            ),
            ("통합과학", &["통합과학"], &["통합과학"]),
            ("한자급수", &["한자급수"], &["한자급수"]),
            ("항공드론", &["항공드론"], &["항공드론"]),
        ] {
            crate::domain::setup::upsert_department(c, &dept(name, aliases, vendors))?;
        }
        Ok(())
    })
    .unwrap();
}

/// 정산자료만으로도 부서 합산·검증이 도는지 (견적서 없이)
#[test]
fn e2e_settlement_only() {
    let Some(dir) = fixtures() else {
        return;
    };
    let settle = dir.join("품의_교재비·재료비_2026학년도_2026년9월_20260911.xlsx");
    if !settle.exists() {
        return;
    }

    let db = memory_db();
    setup_real_departments(&db);
    let work = db
        .write(|c| create_work(c, "2026학년도 9월 교재비", "2026학년도", "9월", "교재비"))
        .unwrap();

    let warnings = db.write(|c| load_settlement(c, work, &settle)).unwrap();
    assert!(
        warnings.iter().all(|w| w.code != "SETTLE_COLUMN_SUM"),
        "정산자료 검산 경고: {warnings:?}"
    );

    let rows = db.read(|c| settlement_rows(c, work)).unwrap();
    assert_eq!(rows.len(), 20);
    // 모든 줄이 부서에 붙어야 한다 (미등록 별칭 0)
    let unmapped: Vec<&SettlementRowView> =
        rows.iter().filter(|r| r.department_id.is_none()).collect();
    assert!(unmapped.is_empty(), "미등록 별칭: {:?}", unmapped.iter().map(|r| &r.source_name).collect::<Vec<_>>());

    // 별칭 여럿이 한 부서로 합산된다
    let depts = db.read(|c| dept_allocations(c, work)).unwrap();
    let craft = depts.iter().find(|d| d.department_name == "토탈공예미니어처").unwrap();
    assert_eq!(craft.settlement.beneficiary, 690_000, "세 별칭 합산");
    let kids = depts.iter().find(|d| d.department_name == "키즈쿠킹").unwrap();
    assert_eq!(kids.settlement.beneficiary, 2_149_000, "두 별칭 합산");
    let baduk = depts.iter().find(|d| d.department_name == "바둑").unwrap();
    assert_eq!(baduk.settlement.total(), 444_000);
}

/// 바둑 견적서 + 정산자료로 끝까지 (XLSX 만 쓰므로 한글 없이도 돈다)
#[test]
fn e2e_baduk_full_flow() {
    let Some(dir) = fixtures() else {
        return;
    };
    let quote = dir.join("바둑 견적서.xlsx");
    let settle = dir.join("품의_교재비·재료비_2026학년도_2026년9월_20260911.xlsx");
    if !quote.exists() || !settle.exists() {
        return;
    }

    let db = memory_db();
    setup_real_departments(&db);
    let work = db
        .write(|c| create_work(c, "2026학년도 9월 교재비", "2026학년도", "9월", "교재비"))
        .unwrap();

    // --- 견적서 등록 (파일명 자동 매칭) ---
    let qid = db.write(|c| register_quote(c, work, &quote)).unwrap();
    let quotes = db.read(|c| load_quotes(c, work)).unwrap();
    let q = quotes.iter().find(|q| q.id == qid).unwrap();
    assert_eq!(q.match_method, "auto", "파일명으로 자동 매칭돼야 한다");
    assert_eq!(q.vendor_mgmt_name, "바둑");
    assert_eq!(q.parse_status, "ok");
    assert_eq!(q.compare_total, 444_000);
    assert_eq!(q.content_phrase, "바둑부 바둑교재(상상바둑) 1종");

    // 원문 보존 확인
    let item = q.items.iter().find(|i| i.kind == RowKind::Item).unwrap();
    assert_eq!(item.raw_name, "바둑교재(상상바둑)");
    assert_eq!(item.display_name, item.raw_name);
    assert!(!item.edited);
    assert!(item.cell_ref.starts_with("견적서!"));

    // --- 정산자료 ---
    db.write(|c| load_settlement(c, work, &settle)).unwrap();

    // --- 배분 (거래처가 하나라 자동) ---
    db.write(|c| apply_auto_allocations(c, work)).unwrap();
    let depts = db.read(|c| dept_allocations(c, work)).unwrap();
    let baduk = depts.iter().find(|d| d.department_name == "바둑").unwrap();
    let v = &baduk.vendors[0];
    assert!(!v.missing);
    assert_eq!(v.allocated.beneficiary, 372_000);
    assert_eq!(v.allocated.excess, 12_000);
    assert_eq!(v.allocated.subsidy, 60_000);
    assert_eq!(v.allocated.voucher, 0);
    assert_eq!(v.allocated.total(), v.quote_total.unwrap(), "가로 검증");

    // --- 검증 ---
    db.write(|c| run_checks(c, work)).unwrap();
    let checks = db.read(|c| list_checks(c, work)).unwrap();
    let baduk_errors: Vec<&CheckRow> = checks
        .iter()
        .filter(|c| c.status == "error" && c.label.contains("바둑"))
        .collect();
    assert!(baduk_errors.is_empty(), "바둑은 오류가 없어야 한다: {baduk_errors:?}");

    // 견적서가 없는 다른 부서들은 '견적 없음' 오류가 난다 — 정상이다
    assert!(checks.iter().any(|c| c.kind == "missing_quote"));

    // --- 미리보기 ---
    let previews = db.read(|c| preview(c, work)).unwrap();
    let by = |f: Fund| previews.iter().find(|p| p.fund == f.key()).unwrap();

    let ben = by(Fund::Beneficiary);
    assert_eq!(ben.rows.len(), 1);
    assert_eq!(ben.rows[0].content, "바둑부 바둑교재(상상바둑) 1종");
    assert_eq!(ben.rows[0].amount, 372_000);

    assert_eq!(by(Fund::Excess).rows.len(), 1);
    assert_eq!(by(Fund::Excess).rows[0].amount, 12_000);
    assert_eq!(by(Fund::Subsidy).rows[0].amount, 60_000);

    // 자유수강권은 0원이라 행이 없다
    let voucher = by(Fund::Voucher);
    assert!(voucher.rows.is_empty(), "0원 재원은 행을 만들지 않는다");
    assert!(voucher.empty_note.contains("파일 생성 안 함"));
}

/// 빨강이 남아 있으면 생성이 막히고, 사유를 적으면 풀린다 (설계안 14장 13번)
#[test]
fn e2e_generation_is_blocked_until_acknowledged() {
    let Some(dir) = fixtures() else { return };
    let quote = dir.join("바둑 견적서.xlsx");
    let settle = dir.join("품의_교재비·재료비_2026학년도_2026년9월_20260911.xlsx");
    if !quote.exists() || !settle.exists() {
        return;
    }

    let db = memory_db();
    // 바둑만 등록한다 → 정산자료의 다른 부서가 '미등록 별칭' 오류가 된다
    db.write(|c| crate::domain::setup::upsert_department(c, &dept("바둑", &["바둑"], &["바둑"])))
        .unwrap();
    let work = db.write(|c| create_work(c, "시험", "2026학년도", "9월", "교재비")).unwrap();
    db.write(|c| register_quote(c, work, &quote)).unwrap();
    db.write(|c| load_settlement(c, work, &settle)).unwrap();
    db.write(|c| apply_auto_allocations(c, work)).unwrap();
    db.write(|c| run_checks(c, work)).unwrap();

    let g = db.read(|c| gate(c, work)).unwrap();
    assert!(!g.can_generate, "미등록 별칭이 있으면 막혀야 한다");
    assert!(!g.open_blockers.is_empty());

    let out = std::env::temp_dir().join("quotemgr-e2e-blocked");
    let e = db.write(|c| generate(c, work, &out)).unwrap_err();
    assert_eq!(e.code, "GENERATE_BLOCKED");

    // 사유 없이 확인하려 하면 거절한다
    let first = g.open_blockers[0].check_id;
    let e = db.write(|c| acknowledge(c, first, "   ")).unwrap_err();
    assert_eq!(e.code, "ACK_REASON_EMPTY");

    // 모든 빨강에 사유를 적으면 풀린다
    db.write(|c| {
        for b in &g.open_blockers {
            acknowledge(c, b.check_id, "이번 달 대상이 아님 — 담당자 확인")?;
        }
        Ok(())
    })
    .unwrap();
    let g2 = db.read(|c| gate(c, work)).unwrap();
    assert!(g2.can_generate);
    assert!(g2.message.contains("사유"));

    // 실제로 만들어진다
    let r = db.write(|c| generate(c, work, &out)).unwrap();
    assert_eq!(r.created.len(), 3, "수익자·초과금·지원금 세 개");
    assert_eq!(r.skipped.len(), 1, "자유수강권은 0원이라 건너뛴다");
    for f in &r.created {
        assert!(std::path::Path::new(&f.path).exists());
    }
    // 생성 기록이 남는다
    let n: i64 = db
        .read(|c| Ok(c.query_row("SELECT count(*) FROM work_output WHERE work_id=?1", [work], |r| r.get(0))?))
        .unwrap();
    assert_eq!(n, 3);

    let _ = std::fs::remove_dir_all(&out);
}

/// 검증을 다시 돌려도 **확인 사유는 남아 있어야** 한다
#[test]
fn acknowledgement_survives_recheck() {
    let Some(dir) = fixtures() else { return };
    let settle = dir.join("품의_교재비·재료비_2026학년도_2026년9월_20260911.xlsx");
    if !settle.exists() {
        return;
    }
    let db = memory_db();
    db.write(|c| crate::domain::setup::upsert_department(c, &dept("바둑", &["바둑"], &["바둑"])))
        .unwrap();
    let work = db.write(|c| create_work(c, "시험", "", "9월", "교재비")).unwrap();
    db.write(|c| load_settlement(c, work, &settle)).unwrap();
    db.write(|c| run_checks(c, work)).unwrap();

    let checks = db.read(|c| list_checks(c, work)).unwrap();
    let target = checks.iter().find(|c| c.kind == "unmapped_alias").unwrap();
    db.write(|c| acknowledge(c, target.id, "다른 달 자료")).unwrap();

    // 다시 검증
    db.write(|c| run_checks(c, work)).unwrap();
    let after = db.read(|c| list_checks(c, work)).unwrap();
    let same = after
        .iter()
        .find(|c| c.kind == "unmapped_alias" && c.label == target.label)
        .unwrap();
    assert!(same.acknowledged, "사유가 남아 있어야 한다");
    assert_eq!(same.ack_reason, "다른 달 자료");
}

/// 원본 파일이 사라져도 **이미 뽑아 둔 자료로 작업이 계속된다** (설계안 14장 15번)
#[test]
fn work_survives_missing_source_file() {
    let Some(dir) = fixtures() else { return };
    let src = dir.join("바둑 견적서.xlsx");
    if !src.exists() {
        return;
    }
    // 샘플 원본은 건드리지 않는다 — 임시 복사본으로 시험한다
    let tmp = std::env::temp_dir().join("quotemgr-e2e-missing");
    std::fs::create_dir_all(&tmp).unwrap();
    let copy = tmp.join("바둑 견적서.xlsx");
    std::fs::copy(&src, &copy).unwrap();

    let db = memory_db();
    db.write(|c| crate::domain::setup::upsert_department(c, &dept("바둑", &["바둑"], &["바둑"])))
        .unwrap();
    let work = db.write(|c| create_work(c, "시험", "", "9월", "교재비")).unwrap();
    db.write(|c| register_quote(c, work, &copy)).unwrap();

    // 원본을 지운다
    std::fs::remove_file(&copy).unwrap();

    let quotes = db.read(|c| load_quotes(c, work)).unwrap();
    assert_eq!(quotes.len(), 1);
    assert!(!quotes[0].source_exists, "원본이 없다고 표시돼야 한다");
    assert_eq!(quotes[0].parse_status, "ok", "이미 읽어 둔 결과는 그대로다");
    assert_eq!(quotes[0].compare_total, 444_000);
    assert_eq!(quotes[0].content_phrase, "바둑부 바둑교재(상상바둑) 1종");

    // 다시 읽기는 안내와 함께 실패한다
    let e = db.write(|c| reparse_quote(c, quotes[0].id)).unwrap_err();
    assert_eq!(e.code, "QUOTE_SOURCE_MISSING");

    let _ = std::fs::remove_dir_all(&tmp);
}

/// 자동으로 못 읽어도 **등록은 되고 작업이 멈추지 않는다** (P3 이후: 글자가 없는 사진)
///
/// P1 때는 JPG 자체가 못 읽는 형식이었다. 이제 사진은 읽으므로,
/// **읽기에 실패했을 때 직접 입력으로 이어지는 길**을 대신 확인한다.
#[test]
fn unreadable_quote_does_not_block_the_work() {
    let tmp = std::env::temp_dir().join(format!("quotemgr-blank-{}", std::process::id()));
    std::fs::create_dir_all(&tmp).unwrap();
    // 글자가 하나도 없는 흰 사진 — OCR 이 알아볼 것이 없다
    let img = tmp.join("주산암산 견적서.jpg");
    image::DynamicImage::ImageRgb8(image::RgbImage::from_pixel(600, 400, image::Rgb([255, 255, 255])))
        .save(&img)
        .unwrap();
    let db = memory_db();
    db.write(|c| {
        crate::domain::setup::upsert_department(c, &dept("주산암산", &["주산암산"], &["주산암산"]))
    })
    .unwrap();
    let work = db.write(|c| create_work(c, "시험", "", "9월", "교재비")).unwrap();
    db.write(|c| register_quote(c, work, &img)).unwrap();

    let quotes = db.read(|c| load_quotes(c, work)).unwrap();
    assert_eq!(quotes.len(), 1, "등록 자체는 남는다");
    assert_eq!(quotes[0].parse_status, "failed");
    assert!(quotes[0].parse_error.contains("직접 입력"), "{}", quotes[0].parse_error);
    assert_eq!(quotes[0].match_method, "auto", "거래처 매칭은 파일명으로 된다");

    // 품목을 손으로 넣을 수 있다
    let item_id = db.write(|c| add_item(c, quotes[0].id)).unwrap();
    db.write(|c| {
        update_item(
            c,
            &ItemEdit {
                id: item_id,
                kind: RowKind::Item,
                sign_effect: None,
                display_name: "방과후 기초Yap! 상".into(),
                spec: "권".into(),
                qty: Some(3),
                unit_price: Some(10_000),
                amount: Some(30_000),
            },
        )
        .map(|_| ())
    })
    .unwrap();

    let quotes = db.read(|c| load_quotes(c, work)).unwrap();
    assert_eq!(quotes[0].item_sum, 30_000);
    assert_eq!(quotes[0].content_phrase, "주산암산부 방과후 기초Yap! 상 1종");
    // 손으로 넣은 줄은 raw 가 비어 있고 edited 표시가 붙는다
    let it = &quotes[0].items[0];
    assert!(it.edited);
    assert_eq!(it.raw_name, "");

    let _ = std::fs::remove_dir_all(&tmp);
}

/// 거래처가 둘이면 **자동 배분하지 않고 막는다**
#[test]
fn two_vendors_require_manual_allocation() {
    let Some(dir) = fixtures() else { return };
    let settle = dir.join("품의_교재비·재료비_2026학년도_2026년9월_20260911.xlsx");
    if !settle.exists() {
        return;
    }
    let db = memory_db();
    db.write(|c| {
        crate::domain::setup::upsert_department(
            c,
            &dept("로봇과학", &["로봇과학"], &["로봇과학1", "로봇과학2"]),
        )
    })
    .unwrap();
    let work = db.write(|c| create_work(c, "시험", "", "9월", "교재비")).unwrap();
    db.write(|c| load_settlement(c, work, &settle)).unwrap();
    db.write(|c| apply_auto_allocations(c, work)).unwrap();

    let depts = db.read(|c| dept_allocations(c, work)).unwrap();
    let robot = depts.iter().find(|d| d.department_name == "로봇과학").unwrap();
    assert_eq!(robot.settlement.beneficiary, 810_000);
    assert_eq!(robot.vendors.len(), 2);
    for v in &robot.vendors {
        assert!(v.missing, "거래처가 둘이면 자동으로 채우지 않는다");
        assert_eq!(v.allocated.total(), 0);
    }

    // 사용자가 넣으면 세로 검증이 돈다
    db.write(|c| {
        save_allocation(
            c,
            work,
            robot.vendors[0].vendor_unit_id,
            Funds { beneficiary: 720_000, excess: 6_200, subsidy: 173_800, voucher: 0 },
            false,
        )?;
        save_allocation(
            c,
            work,
            robot.vendors[1].vendor_unit_id,
            Funds { beneficiary: 90_000, excess: 0, subsidy: 270_000, voucher: 0 },
            false,
        )
    })
    .unwrap();

    db.write(|c| run_checks(c, work)).unwrap();
    let checks = db.read(|c| list_checks(c, work)).unwrap();
    let vertical_errors: Vec<&CheckRow> = checks
        .iter()
        .filter(|c| c.kind == "vertical" && c.status == "error")
        .collect();
    assert!(vertical_errors.is_empty(), "세로가 맞아야 한다: {vertical_errors:?}");
}

/// 로봇과학 HWP 를 포함한 전체 흐름 (한글 필요)
#[test]
#[ignore = "한글이 깔린 PC 에서만 돈다"]
fn e2e_with_hwp() {
    let Some(dir) = fixtures() else { return };
    let hwp = dir.join("로봇과학 견적서.hwp");
    let baduk = dir.join("바둑 견적서.xlsx");
    let settle = dir.join("품의_교재비·재료비_2026학년도_2026년9월_20260911.xlsx");
    if !hwp.exists() || !settle.exists() {
        return;
    }

    let db = memory_db();
    setup_real_departments(&db);
    let work = db
        .write(|c| create_work(c, "2026학년도 9월 교재비", "2026학년도", "9월", "교재비"))
        .unwrap();

    db.write(|c| register_quote(c, work, &hwp)).unwrap();
    db.write(|c| register_quote(c, work, &baduk)).unwrap();
    db.write(|c| load_settlement(c, work, &settle)).unwrap();
    db.write(|c| apply_auto_allocations(c, work)).unwrap();

    let quotes = db.read(|c| load_quotes(c, work)).unwrap();
    let robot = quotes.iter().find(|q| q.source_name.contains("로봇과학")).unwrap();
    assert_eq!(robot.parse_status, "ok");
    assert!(robot.table_note.contains("견적서"), "납품서가 아니라 견적서를 골라야 한다");
    assert_eq!(robot.item_sum, 900_000);
    assert_eq!(robot.compare_total, 900_000);
    assert_eq!(robot.content_phrase, "로봇과학부 프로보테크닉 교구 외 1종");
    assert_eq!(robot.items.iter().filter(|i| i.kind == RowKind::Item).count(), 2);

    // 로봇과학은 거래처가 하나로 등록돼 있으므로 정산 금액이 그대로 배분되고,
    // 견적(900,000)과 정산(1,260,000)이 달라 **가로 검증이 어긋난다** — 실제로 그렇다.
    db.write(|c| run_checks(c, work)).unwrap();
    let checks = db.read(|c| list_checks(c, work)).unwrap();
    let h = checks
        .iter()
        .find(|c| c.kind == "horizontal" && c.label.contains("로봇과학"))
        .unwrap();
    assert_eq!(h.status, "error");
    assert_eq!(h.diff, Some(1_260_000 - 900_000), "차액을 그대로 보여 준다");
    println!("로봇과학 가로 검증: {} (차이 {:?})", h.label, h.diff);

    // 바둑은 맞는다
    let baduk_h = checks
        .iter()
        .find(|c| c.kind == "horizontal" && c.label.contains("바둑"))
        .unwrap();
    assert_eq!(baduk_h.status, "ok");
}

/// P3 — 사진 견적서가 **작업 전체 흐름**을 그대로 지나간다.
/// 등록 → 거래처 매칭 → 읽기 → 저장 → 정산 대조 → 검증까지 DB 를 거쳐 확인한다.
#[test]
#[ignore = "Windows 한국어 OCR 이 있어야 돈다"]
fn e2e_with_photo_quote() {
    let Some(dir) = fixtures() else { return };
    let jpg = dir.join("주산암산 견적서.jpg");
    let settle = dir.join("품의_교재비·재료비_2026학년도_2026년9월_20260911.xlsx");
    if !jpg.exists() || !settle.exists() {
        return;
    }

    let db = memory_db();
    setup_real_departments(&db);
    let work = db
        .write(|c| create_work(c, "2026학년도 9월 교재비", "2026학년도", "9월", "교재비"))
        .unwrap();

    db.write(|c| register_quote(c, work, &jpg)).unwrap();
    db.write(|c| load_settlement(c, work, &settle)).unwrap();
    db.write(|c| apply_auto_allocations(c, work)).unwrap();

    let quotes = db.read(|c| load_quotes(c, work)).unwrap();
    let q = quotes.iter().find(|q| q.source_name.contains("주산암산")).unwrap();

    // 파일명으로 거래처가 잡히고, 사진에서 읽었다는 사실이 남는다
    assert_eq!(q.match_method, "auto");
    assert_eq!(q.parse_status, "ok");
    assert_eq!(q.source, "ocr", "사진에서 읽었다는 것이 DB 에 남아야 한다");
    assert_eq!(q.trust, "needs_check", "사진은 사람이 봐야 한다");

    assert_eq!(q.items.iter().filter(|i| i.kind == RowKind::Item).count(), 4);
    assert_eq!(q.item_sum, 68_000);
    assert_eq!(q.compare_total, 68_000, "깨져 읽힌 합계(8,000) 대신 품목 합을 쓴다");
    assert_eq!(q.content_phrase, "주산암산부 방과후 기초Yap! 상 외 3종");

    // 값은 고쳐지지 않고 원문이 그대로 남는다
    assert_eq!(q.grand_total, Some(8_000), "읽은 값은 지우지 않는다");
    assert_eq!(q.raw_grand_total.trim(), "鬧8,000", "원문도 남는다");
    assert!(q.warnings.iter().any(|w| w.code == "OCR_TOTAL_UNREADABLE"));
    for it in q.items.iter().filter(|i| i.kind == RowKind::Item) {
        assert_eq!(it.confidence, crate::domain::Confidence::Medium);
    }

    // 정산자료 주산암산 = 40,000 + 0 + 18,000 + 10,000 = 68,000 → 가로 검증이 맞는다
    db.write(|c| run_checks(c, work)).unwrap();
    let checks = db.read(|c| list_checks(c, work)).unwrap();
    let h = checks
        .iter()
        .find(|c| c.kind == "horizontal" && c.label.contains("주산암산"))
        .unwrap();
    assert_eq!(h.status, "ok", "{} {:?}", h.label, h.diff);
}

// ------------------------------------------------------------------ P4-7 전체 회귀

/// 시험 산출물 자리. **샘플 원본은 절대 건드리지 않는다.**
fn out_dir(name: &str) -> std::path::PathBuf {
    let d = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .join("test")
        .join("out")
        .join(format!("{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

/// **P4-7 전체 End-to-End.**
///
/// 네 가지 형식(XLSX · HWP · 사진 · PDF)을 한 작업에 함께 넣고,
/// 정산자료 해석 → 배분 → 세로·가로 검증 → 확인 사유 → 파일 생성까지 간다.
/// 마지막으로 **DB 를 닫았다가 다시 열어** 작업이 그대로 이어지는지 본다.
#[test]
#[ignore = "한글 COM 과 Windows 한국어 OCR 이 모두 있어야 돈다 (1분쯤 걸린다)"]
fn p4_full_end_to_end() {
    let Some(dir) = fixtures() else {
        return;
    };
    let xlsx = dir.join("바둑 견적서.xlsx");
    let hwp = dir.join("로봇과학 견적서.hwp");
    let jpg = dir.join("주산암산 견적서.jpg");
    let settle = dir.join("품의_교재비·재료비_2026학년도_2026년9월_20260911.xlsx");
    if !xlsx.exists() || !hwp.exists() || !jpg.exists() || !settle.exists() {
        eprintln!("샘플이 모자람 — 건너뜀");
        return;
    }

    let work_dir = out_dir("p4-e2e");
    // 텍스트 PDF 는 한글로 만들어 쓴다 (업무에서 받은 PDF 샘플이 아직 없다).
    // 다른 부서 이름을 붙여 **네 번째 거래처**로 등록한다.
    let pdf = work_dir.join("한자급수 견적서.pdf");
    crate::quote::hwp::save_as(&hwp, &pdf, "PDF").expect("한글로 PDF 내보내기");

    let db_path = work_dir.join("quotemgr.db");
    let outputs = work_dir.join("생성");

    // ---------------------------------------------------------------- 1차: 만들기
    let created_ids;
    {
        let db = crate::db::Db::open(&db_path).expect("자료 파일 열기");
        setup_real_departments(&db);
        let work = db
            .write(|c| create_work(c, "2026학년도 9월 교재비", "2026학년도", "9월", "교재비"))
            .unwrap();

        for p in [&xlsx, &hwp, &jpg, &pdf] {
            db.write(|c| register_quote(c, work, p)).unwrap_or_else(|e| {
                panic!("{} 등록 실패: {}", p.file_name().unwrap().to_string_lossy(), e.message)
            });
        }

        let quotes = db.read(|c| load_quotes(c, work)).unwrap();
        assert_eq!(quotes.len(), 4, "네 가지 형식이 모두 등록돼야 한다");

        // 형식마다 읽은 경로가 제대로 기록된다
        let by = |needle: &str| quotes.iter().find(|q| q.source_name.contains(needle)).unwrap();
        assert_eq!(by("바둑").source, "structured");
        assert_eq!(by("로봇과학").source, "structured");
        assert_eq!(by("주산암산").source, "ocr");
        assert_eq!(by("한자급수").source, "pdf_text");

        // 모두 파일명으로 거래처가 잡힌다
        for q in &quotes {
            assert_eq!(q.match_method, "auto", "{}", q.source_name);
            assert_eq!(q.parse_status, "ok", "{} — {}", q.source_name, q.parse_error);
        }

        // 값은 형식과 상관없이 같은 규칙으로 나온다
        assert_eq!(by("바둑").compare_total, 444_000);
        assert_eq!(by("로봇과학").compare_total, 900_000);
        assert_eq!(by("주산암산").compare_total, 68_000);
        assert_eq!(by("주산암산").trust, "needs_check", "사진은 사람이 봐야 한다");
        assert_eq!(by("바둑").content_phrase, "바둑부 바둑교재(상상바둑) 1종");
        assert_eq!(by("주산암산").content_phrase, "주산암산부 방과후 기초Yap! 상 외 3종");

        // ------------------------------------------------------------ 정산자료 · 배분
        let warns = db.write(|c| load_settlement(c, work, &settle)).unwrap();
        println!("정산자료 안내 {}건", warns.len());
        db.write(|c| apply_auto_allocations(c, work)).unwrap();

        // 정산 별칭 여러 개가 한 부서로 합쳐진다 (토탈공예미니어처 1·2)
        let depts = db.read(|c| dept_allocations(c, work)).unwrap();
        let total_craft = depts.iter().find(|d| d.department_name == "토탈공예미니어처").unwrap();
        assert!(total_craft.settlement.total() > 0, "별칭 여러 개가 합산돼야 한다");

        // 거래처가 하나인 부서는 자동 배분된다
        let baduk = depts.iter().find(|d| d.department_name == "바둑").unwrap();
        assert!(!baduk.vendors[0].missing);
        assert_eq!(baduk.vendors[0].allocated.total(), 444_000, "가로 검증이 맞는다");
        // 자유수강권 0원
        assert_eq!(baduk.vendors[0].allocated.voucher, 0);

        // 주산암산도 맞는다 (사진에서 읽은 68,000 = 정산 68,000)
        let jusan = depts.iter().find(|d| d.department_name == "주산암산").unwrap();
        assert_eq!(jusan.vendors[0].allocated.total(), 68_000);

        // ------------------------------------------------------------ 검증 · 확인 사유
        db.write(|c| run_checks(c, work)).unwrap();
        let checks = db.read(|c| list_checks(c, work)).unwrap();
        let errors: Vec<&CheckRow> = checks.iter().filter(|c| c.status == "error").collect();
        assert!(!errors.is_empty(), "견적서가 없는 부서가 있으므로 빨강이 남는다");
        println!("빨강 {}건", errors.len());

        // 빨강이 있으면 생성이 막힌다
        let g = db.read(|c| gate(c, work)).unwrap();
        assert!(!g.can_generate, "{}", g.message);
        let blocked = db.write(|c| generate(c, work, &outputs)).unwrap_err();
        assert_eq!(blocked.code, "GENERATE_BLOCKED");

        // 사유를 적으면 풀린다
        for e in &errors {
            db.write(|c| acknowledge(c, e.id, "부서와 확인함 (P4 회귀 시험)")).unwrap();
        }
        let g = db.read(|c| gate(c, work)).unwrap();
        assert!(g.can_generate, "{}", g.message);

        // ------------------------------------------------------------ 생성
        let result = db.write(|c| generate(c, work, &outputs)).unwrap();
        assert!(!result.created.is_empty(), "파일이 하나는 만들어져야 한다");
        for f in &result.created {
            let p = std::path::Path::new(&f.path);
            assert!(p.exists(), "{}", f.path);
            println!("만듦: {} · {}줄 · {}원", f.fund_label, f.row_count, f.total);
        }
        // 재원 넷을 모두 따져 본다 — 만들었거나, 0원이라 건너뛰었거나 둘 중 하나다
        assert_eq!(result.created.len() + result.skipped.len(), 4, "{:?}", result.skipped);
        for s in &result.skipped {
            assert!(s.contains("파일 생성 안 함"), "{s}");
        }
        // (0원 재원이 실제로 건너뛰어지는지는 `e2e_baduk_full_flow` 가 따로 본다)

        created_ids = (work, result.created.len());
    } // ← DB 를 닫는다 (프로그램을 끈 것과 같다)

    // ---------------------------------------------------------------- 2차: 이어하기
    {
        let db = crate::db::Db::open(&db_path).expect("자료 파일 다시 열기");
        let (work, made) = created_ids;

        let works = db.read(|c| list_works(c)).unwrap();
        assert_eq!(works.len(), 1, "작업이 그대로 있어야 한다");
        assert_eq!(works[0].id, work);

        let quotes = db.read(|c| load_quotes(c, work)).unwrap();
        assert_eq!(quotes.len(), 4, "견적서 네 장이 그대로");
        let jusan = quotes.iter().find(|q| q.source_name.contains("주산암산")).unwrap();
        assert_eq!(jusan.item_sum, 68_000, "사진에서 읽은 값이 남아 있다 (다시 읽지 않는다)");
        assert_eq!(jusan.source, "ocr");
        assert_eq!(jusan.items.iter().filter(|i| i.kind == RowKind::Item).count(), 4);

        // 확인 사유도 남는다
        let checks = db.read(|c| list_checks(c, work)).unwrap();
        assert!(checks.iter().any(|c| c.ack_reason.contains("P4 회귀 시험")));
        let g = db.read(|c| gate(c, work)).unwrap();
        assert!(g.can_generate, "껐다 켜도 생성할 수 있어야 한다: {}", g.message);

        // 만든 파일 기록도 남는다
        let files: Vec<_> = std::fs::read_dir(&outputs).unwrap().flatten().collect();
        assert_eq!(files.len(), made, "만든 파일이 그대로 있다");
    }

    println!("P4 전체 흐름 통과 — {}", work_dir.display());
}
