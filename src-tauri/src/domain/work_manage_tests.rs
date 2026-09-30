//! v0.1.1 작업 관리 시험 — 이름 짓기·이름 바꾸기·지우기.
//!
//! 여기서 지키는 것은 셋이다.
//!   1. 이름 없는 작업은 만들지도 바꾸지도 못한다.
//!   2. **이름을 바꿔도 작업 id 와 딸린 자료는 하나도 변하지 않는다.**
//!   3. 작업을 지우면 그 작업 자료만 사라지고, 다른 작업과 **원본 파일**은 그대로다.

use super::*;
use crate::db::memory_db;
use crate::db::Db;

/// 작업 하나에 딸린 자료를 표마다 한 줄씩 심는다.
///
/// 실제 견적서를 읽지 않고 SQL 로 바로 넣는다. 여기서 보려는 것은 파싱이 아니라
/// **지울 때 어느 표까지 따라 지워지는가**이기 때문이다.
fn seed_work(db: &Db, title: &str, vendor_unit_id: i64, source_path: &str) -> i64 {
    db.write(|c| {
        let work = create_work(c, title, "2026학년도", "9월", "교재비")?;
        c.execute(
            "INSERT INTO work_quote(work_id, vendor_unit_id, source_path, source_name, format, parse_status, grand_total)
             VALUES (?1, ?2, ?3, '견적서.xlsx', 'xlsx', 'ok', 444000)",
            params![work, vendor_unit_id, source_path],
        )?;
        let quote = c.last_insert_rowid();
        c.execute(
            "INSERT INTO work_quote_item(work_quote_id, row_no, kind, display_name, raw_name, amount)
             VALUES (?1, 0, 'item', '바둑교재', '바둑교재', 444000)",
            params![quote],
        )?;
        c.execute(
            "INSERT INTO work_settlement_row(work_id, row_no, source_name, beneficiary)
             VALUES (?1, 0, '바둑', 444000)",
            params![work],
        )?;
        c.execute(
            "INSERT INTO work_allocation(work_id, vendor_unit_id, beneficiary, source, updated_at)
             VALUES (?1, ?2, 444000, 'auto', '2026-09-30T00:00:00')",
            params![work, vendor_unit_id],
        )?;
        c.execute(
            "INSERT INTO work_check(work_id, kind, label, status)
             VALUES (?1, 'vertical', '바둑', 'ok')",
            params![work],
        )?;
        c.execute(
            "INSERT INTO work_output(work_id, fund, path, row_count, total, created_at)
             VALUES (?1, 'beneficiary', 'D:/품의/수익자.xlsx', 1, 444000, '2026-09-30T00:00:00')",
            params![work],
        )?;
        Ok(work)
    })
    .expect("자료 심기")
}

/// 작업에 매달린 표들. 지울 때 여기 전부가 함께 치워져야 한다.
const CHILD_TABLES: &[&str] = &[
    "work_quote",
    "work_settlement_row",
    "work_allocation",
    "work_check",
    "work_output",
];

fn count_for_work(db: &Db, table: &str, work: i64) -> i64 {
    db.read(|c| {
        Ok(c.query_row(&format!("SELECT count(*) FROM {table} WHERE work_id = ?1"), [work], |r| {
            r.get(0)
        })?)
    })
    .unwrap()
}

fn item_count(db: &Db, work: i64) -> i64 {
    db.read(|c| {
        Ok(c.query_row(
            "SELECT count(*) FROM work_quote_item i
               JOIN work_quote q ON q.id = i.work_quote_id
              WHERE q.work_id = ?1",
            [work],
            |r| r.get(0),
        )?)
    })
    .unwrap()
}

fn two_departments(db: &Db) -> (i64, i64) {
    use crate::domain::setup::{Department, VendorUnit};
    let mk = |name: &str| Department {
        id: 0,
        display_name: name.into(),
        phrase_name: format!("{name}부"),
        sort_order: 0,
        active: true,
        aliases: vec![name.into()],
        vendors: vec![VendorUnit {
            id: 0,
            mgmt_name: name.into(),
            vendor_name: String::new(),
            note: String::new(),
            sort_order: 0,
            active: true,
        }],
    };
    db.write(|c| {
        crate::domain::setup::upsert_department(c, &mk("바둑"))?;
        crate::domain::setup::upsert_department(c, &mk("로봇과학"))?;
        let ids: Vec<i64> = crate::domain::setup::list_departments(c)?
            .iter()
            .flat_map(|d| d.vendors.iter().map(|v| v.id))
            .collect();
        Ok((ids[0], ids[1]))
    })
    .unwrap()
}

// ---------------------------------------------------------------- 이름 짓기

#[test]
fn new_work_keeps_the_name_it_was_given() {
    let db = memory_db();
    let id = db.write(|c| create_work(c, "9월 재료비 품의", "2026학년도", "9월", "재료비")).unwrap();
    assert_eq!(db.read(|c| get_work(c, id)).unwrap().title, "9월 재료비 품의");
}

#[test]
fn new_work_name_is_trimmed() {
    let db = memory_db();
    let id = db.write(|c| create_work(c, "  9월 교재비  ", "", "", "")).unwrap();
    assert_eq!(db.read(|c| get_work(c, id)).unwrap().title, "9월 교재비");
}

#[test]
fn blank_names_are_refused() {
    let db = memory_db();
    for bad in ["", "   ", "\t", "\u{3000}", " \r\n "] {
        let e = db.write(|c| create_work(c, bad, "", "", "")).unwrap_err();
        assert_eq!(e.code, "WORK_TITLE_EMPTY", "만들기: {bad:?} 는 막혀야 한다");
    }
    let id = db.write(|c| create_work(c, "제대로 된 이름", "", "", "")).unwrap();
    for bad in ["", "   ", "\t", "\u{3000}"] {
        let e = db.write(|c| rename_work(c, id, bad)).unwrap_err();
        assert_eq!(e.code, "WORK_TITLE_EMPTY", "이름 바꾸기: {bad:?} 는 막혀야 한다");
    }
    assert_eq!(
        db.read(|c| get_work(c, id)).unwrap().title,
        "제대로 된 이름",
        "막힌 뒤에도 원래 이름이 그대로여야 한다"
    );
}

// ---------------------------------------------------------------- 이름 바꾸기

#[test]
fn rename_changes_only_the_name() {
    let db = memory_db();
    let (baduk, _) = two_departments(&db);
    let work = seed_work(&db, "이름 바꾸기 전", baduk, "D:/견적/바둑 견적서.xlsx");

    let before = db.read(|c| get_work(c, work)).unwrap();
    db.write(|c| rename_work(c, work, "  2026학년도 9월 교재비  ")).unwrap();
    let after = db.read(|c| get_work(c, work)).unwrap();

    assert_eq!(after.id, before.id, "작업 id 는 절대 바뀌지 않는다");
    assert_eq!(after.title, "2026학년도 9월 교재비", "앞뒤 공백은 떼고 저장한다");
    assert_eq!(after.school_year, before.school_year);
    assert_eq!(after.month, before.month);
    assert_eq!(after.kind, before.kind);
    assert_eq!(after.created_at, before.created_at, "만든 때는 그대로다");
    assert_eq!(after.status, before.status);

    // 딸린 자료가 하나도 줄지 않는다
    for t in CHILD_TABLES {
        assert_eq!(count_for_work(&db, t, work), 1, "{t} 가 그대로 있어야 한다");
    }
    assert_eq!(item_count(&db, work), 1, "추출 결과가 그대로 있어야 한다");
}

#[test]
fn rename_of_a_missing_work_is_refused() {
    let db = memory_db();
    let e = db.write(|c| rename_work(c, 9999, "없는 작업")).unwrap_err();
    assert_eq!(e.code, "WORK_NOT_FOUND");
}

/// 껐다 켠 뒤에도 이름이 남는가 — 메모리가 아니라 **파일 DB** 로 본다.
#[test]
fn names_survive_a_restart() {
    let dir = std::env::temp_dir().join(format!("purchase-helper-test-{}-rename", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("rename-restart.db");
    let _ = std::fs::remove_file(&path);

    let work = {
        let db = Db::open(&path).unwrap();
        let id = db.write(|c| create_work(c, "처음 지은 이름", "2026학년도", "9월", "교재비")).unwrap();
        db.write(|c| rename_work(c, id, "나중에 바꾼 이름")).unwrap();
        id
    }; // ← 프로그램을 끈 것과 같다

    {
        let db = Db::open(&path).unwrap();
        let w = db.read(|c| get_work(c, work)).unwrap();
        assert_eq!(w.id, work, "id 가 그대로다");
        assert_eq!(w.title, "나중에 바꾼 이름", "껐다 켜도 바뀐 이름이 남아야 한다");
    }

    let _ = std::fs::remove_dir_all(&dir);
}

// ---------------------------------------------------------------- 지우기

#[test]
fn delete_removes_this_work_only() {
    let db = memory_db();
    let (baduk, robot) = two_departments(&db);
    let gone = seed_work(&db, "지울 작업", baduk, "D:/견적/바둑 견적서.xlsx");
    let kept = seed_work(&db, "남길 작업", robot, "D:/견적/로봇과학 견적서.xlsx");

    db.write(|c| delete_work(c, gone)).unwrap();

    // 지운 작업 자료는 한 줄도 남지 않는다
    assert!(db.read(|c| get_work(c, gone)).is_err(), "작업이 사라져야 한다");
    for t in CHILD_TABLES {
        assert_eq!(count_for_work(&db, t, gone), 0, "{t} 에 지운 작업 자료가 남았다");
    }
    assert_eq!(item_count(&db, gone), 0, "추출 결과가 남았다");

    // 고아 행이 남지 않았는지 표 전체로 다시 확인한다
    let orphans: i64 = db
        .read(|c| {
            Ok(c.query_row(
                "SELECT count(*) FROM work_quote_item i
                   LEFT JOIN work_quote q ON q.id = i.work_quote_id
                  WHERE q.id IS NULL",
                [],
                |r| r.get(0),
            )?)
        })
        .unwrap();
    assert_eq!(orphans, 0, "어느 견적서에도 매달리지 않은 추출 행이 남았다");

    // 남긴 작업은 하나도 건드리지 않았다
    assert_eq!(db.read(|c| get_work(c, kept)).unwrap().title, "남길 작업");
    for t in CHILD_TABLES {
        assert_eq!(count_for_work(&db, t, kept), 1, "{t} 에서 남길 작업 자료가 사라졌다");
    }
    assert_eq!(item_count(&db, kept), 1);

    // 부서·거래처 설정은 작업에 딸린 것이 아니므로 그대로 남는다
    assert_eq!(db.read(crate::domain::setup::list_departments).unwrap().len(), 2);

    assert_eq!(db.read(list_works).unwrap().len(), 1);
}

#[test]
fn delete_of_a_missing_work_is_refused() {
    let db = memory_db();
    let e = db.write(|c| delete_work(c, 9999)).unwrap_err();
    assert_eq!(e.code, "WORK_NOT_FOUND");
}

/// **원본 견적서 파일은 지우지 않는다.** 앱이 만든 파일이 아니다 (설계안 14장 15번).
#[test]
fn delete_never_touches_the_original_files() {
    let dir =
        std::env::temp_dir().join(format!("purchase-helper-test-{}-files", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let source = dir.join("바둑 견적서.xlsx");
    let output = dir.join("수익자부담.xlsx");
    std::fs::write(&source, b"pretend-quote").unwrap();
    std::fs::write(&output, b"pretend-output").unwrap();

    let db = memory_db();
    let (baduk, _) = two_departments(&db);
    let work = seed_work(&db, "지울 작업", baduk, &source.to_string_lossy());
    db.write(|c| {
        c.execute(
            "UPDATE work_output SET path = ?2 WHERE work_id = ?1",
            params![work, output.to_string_lossy()],
        )?;
        Ok(())
    })
    .unwrap();

    db.write(|c| delete_work(c, work)).unwrap();

    assert!(source.exists(), "원본 견적서를 지우면 안 된다");
    assert!(output.exists(), "만들어 둔 품의 파일을 지우면 안 된다");

    let _ = std::fs::remove_dir_all(&dir);
}

/// 자료를 지우는 일은 **연결마다 켜 둔 외래 키 설정**에 기대고 있다.
/// 이 설정이 꺼지면 딸린 자료가 고아로 남으므로 여기서 못을 박는다.
#[test]
fn foreign_keys_are_on() {
    let dir = std::env::temp_dir().join(format!("purchase-helper-test-{}-fk", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("fk.db");
    let _ = std::fs::remove_file(&path);

    let db = Db::open(&path).unwrap();
    let on: i64 = db.read(|c| Ok(c.query_row("PRAGMA foreign_keys", [], |r| r.get(0))?)).unwrap();
    assert_eq!(on, 1, "외래 키 설정이 켜져 있어야 딸린 자료가 함께 지워진다");

    drop(db);
    let _ = std::fs::remove_dir_all(&dir);
}
