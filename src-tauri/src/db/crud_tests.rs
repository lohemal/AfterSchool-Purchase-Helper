//! P1-1 DB 기본 시험 — 표가 서로 제대로 묶여 있는지, 제약이 도는지.

use super::*;
use crate::domain::setup::{Department, VendorUnit};

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

#[test]
fn schema_is_current() {
    let db = memory_db();
    assert_eq!(db.schema_version().unwrap(), migrate::latest_version());
}

#[test]
fn department_with_aliases_and_vendors() {
    let db = memory_db();
    let id = db
        .write(|c| {
            crate::domain::setup::upsert_department(
                c,
                &dept(
                    "토탈공예미니어처",
                    &["토탈공예미니어처", "토탈공예미니어처1", "토탈공예미니어처2"],
                    &["토탈공예"],
                ),
            )
        })
        .unwrap();
    assert!(id > 0);

    let list = db.read(|c| crate::domain::setup::list_departments(c)).unwrap();
    assert_eq!(list.len(), 1);
    assert_eq!(list[0].display_name, "토탈공예미니어처");
    assert_eq!(list[0].phrase_name, "토탈공예미니어처부");
    assert_eq!(list[0].aliases.len(), 3, "정산 별칭 3개");
    assert_eq!(list[0].vendors.len(), 1, "거래처 1개");
}

/// 정산 별칭과 거래처 관리명은 **다른 표**다. 같은 글자여도 서로 간섭하지 않는다.
#[test]
fn alias_and_vendor_are_separate_concepts() {
    let db = memory_db();
    db.write(|c| {
        crate::domain::setup::upsert_department(
            c,
            &dept("로봇과학", &["로봇과학"], &["로봇과학1", "로봇과학2"]),
        )
    })
    .unwrap();

    let d = &db.read(|c| crate::domain::setup::list_departments(c)).unwrap()[0];
    assert_eq!(d.aliases, vec!["로봇과학"], "정산 별칭은 부서 단위 하나뿐");
    assert_eq!(
        d.vendors.iter().map(|v| v.mgmt_name.clone()).collect::<Vec<_>>(),
        vec!["로봇과학1", "로봇과학2"],
        "거래처 관리명은 둘"
    );

    // 매칭 후보에는 거래처만 들어간다 (별칭은 절대 안 들어간다)
    let cands = db.read(|c| crate::domain::work::match_candidates(c)).unwrap();
    assert_eq!(cands.len(), 2);
    assert!(cands.iter().all(|c| c.mgmt_name.starts_with("로봇과학")));
    assert!(cands.iter().all(|c| c.department_name == "로봇과학"));
}

#[test]
fn duplicate_names_are_refused_with_korean_message() {
    let db = memory_db();
    db.write(|c| crate::domain::setup::upsert_department(c, &dept("바둑", &["바둑"], &["바둑"])))
        .unwrap();

    let e = db
        .write(|c| crate::domain::setup::upsert_department(c, &dept("바둑", &[], &[])))
        .unwrap_err();
    assert_eq!(e.code, "DEPT_DUPLICATE");
    assert!(e.message.contains("이미"), "{}", e.message);

    // 다른 부서가 같은 별칭을 쓰려 하면 막는다
    let e = db
        .write(|c| crate::domain::setup::upsert_department(c, &dept("체스", &["바둑"], &["체스"])))
        .unwrap_err();
    assert_eq!(e.code, "ALIAS_DUPLICATE");
}

#[test]
fn deleting_department_cascades() {
    let db = memory_db();
    let id = db
        .write(|c| crate::domain::setup::upsert_department(c, &dept("바둑", &["바둑"], &["바둑"])))
        .unwrap();
    db.write(|c| crate::domain::setup::delete_department(c, id)).unwrap();

    let n: i64 = db
        .read(|c| Ok(c.query_row("SELECT count(*) FROM settlement_alias", [], |r| r.get(0))?))
        .unwrap();
    assert_eq!(n, 0, "별칭도 함께 지워져야 한다");
    let n: i64 = db
        .read(|c| Ok(c.query_row("SELECT count(*) FROM vendor_unit", [], |r| r.get(0))?))
        .unwrap();
    assert_eq!(n, 0, "거래처도 함께 지워져야 한다");
}

/// 부서를 고쳐도 **거래처 id 는 유지된다** (견적서 연결이 끊기면 안 된다)
#[test]
fn editing_keeps_vendor_ids() {
    let db = memory_db();
    let id = db
        .write(|c| {
            crate::domain::setup::upsert_department(c, &dept("로봇과학", &["로봇과학"], &["로봇과학1"]))
        })
        .unwrap();
    let before = db.read(|c| crate::domain::setup::list_departments(c)).unwrap()[0].vendors[0].id;

    // 거래처를 하나 더하면서 저장
    let mut d = db.read(|c| crate::domain::setup::list_departments(c)).unwrap()[0].clone();
    d.id = id;
    d.vendors.push(VendorUnit {
        id: 0,
        mgmt_name: "로봇과학2".into(),
        vendor_name: String::new(),
        note: String::new(),
        sort_order: 1,
        active: true,
    });
    db.write(|c| crate::domain::setup::upsert_department(c, &d)).unwrap();

    let after = db.read(|c| crate::domain::setup::list_departments(c)).unwrap();
    assert_eq!(after[0].vendors.len(), 2);
    assert_eq!(after[0].vendors[0].id, before, "기존 거래처 id 가 유지돼야 한다");
}

#[test]
fn setup_export_import_round_trip() {
    let db = memory_db();
    db.write(|c| {
        crate::domain::setup::upsert_department(
            c,
            &dept("키즈쿠킹", &["목요키즈쿠킹", "화요키즈쿠킹"], &["키즈쿠킹"]),
        )?;
        crate::domain::setup::upsert_department(c, &dept("바둑", &["바둑"], &["바둑"]))
    })
    .unwrap();

    let exported = db.read(|c| crate::domain::setup::export_setup(c)).unwrap();
    assert_eq!(exported.departments.len(), 2);
    let json = serde_json::to_string(&exported).unwrap();

    // 새 DB 에 넣어 본다
    let db2 = memory_db();
    let parsed: crate::domain::setup::SetupExport = serde_json::from_str(&json).unwrap();
    let n = db2.write(|c| crate::domain::setup::import_setup(c, &parsed)).unwrap();
    assert_eq!(n, 2);

    let back = db2.read(|c| crate::domain::setup::list_departments(c)).unwrap();
    let kids = back.iter().find(|d| d.display_name == "키즈쿠킹").unwrap();
    assert_eq!(kids.aliases.len(), 2);
    assert_eq!(kids.phrase_name, "키즈쿠킹부");
}

#[test]
fn import_refuses_foreign_format() {
    let db = memory_db();
    let bad = crate::domain::setup::SetupExport {
        format: "something-else".into(),
        exported_at: String::new(),
        departments: vec![],
    };
    let e = db.write(|c| crate::domain::setup::import_setup(c, &bad)).unwrap_err();
    assert_eq!(e.code, "SETUP_FORMAT");
}

#[test]
fn alias_resolution() {
    let db = memory_db();
    let id = db
        .write(|c| {
            crate::domain::setup::upsert_department(
                c,
                &dept("토탈공예미니어처", &["토탈공예미니어처1"], &["토탈공예"]),
            )
        })
        .unwrap();
    let got = db
        .read(|c| crate::domain::setup::resolve_alias(c, "토탈공예미니어처1"))
        .unwrap();
    assert_eq!(got, Some(id));
    // 등록하지 않은 이름은 **추측하지 않는다**
    let got = db
        .read(|c| crate::domain::setup::resolve_alias(c, "토탈공예미니어처2"))
        .unwrap();
    assert_eq!(got, None, "숫자만 다른 이름을 멋대로 같다고 보면 안 된다");
}

#[test]
fn settings_round_trip() {
    let db = memory_db();
    db.write(|c| set_setting(c, "last_output_dir", "D:/품의")).unwrap();
    let v = db.read(|c| get_setting(c, "last_output_dir")).unwrap();
    assert_eq!(v.as_deref(), Some("D:/품의"));
    db.write(|c| set_setting(c, "last_output_dir", "E:/다른곳")).unwrap();
    let v = db.read(|c| get_setting(c, "last_output_dir")).unwrap();
    assert_eq!(v.as_deref(), Some("E:/다른곳"), "덮어쓰기");
    assert!(db.read(|c| get_setting(c, "없는키")).unwrap().is_none());
}

#[test]
fn work_crud() {
    let db = memory_db();
    let id = db
        .write(|c| crate::domain::work::create_work(c, "2026학년도 9월 교재비", "2026학년도", "9월", "교재비"))
        .unwrap();
    let w = db.read(|c| crate::domain::work::get_work(c, id)).unwrap();
    assert_eq!(w.title, "2026학년도 9월 교재비");
    assert_eq!(w.month, "9월");
    assert_eq!(w.status, "open");

    assert_eq!(db.read(|c| crate::domain::work::list_works(c)).unwrap().len(), 1);
    db.write(|c| crate::domain::work::delete_work(c, id)).unwrap();
    assert_eq!(db.read(|c| crate::domain::work::list_works(c)).unwrap().len(), 0);

    let e = db
        .write(|c| crate::domain::work::create_work(c, "  ", "", "", ""))
        .unwrap_err();
    assert_eq!(e.code, "WORK_TITLE_EMPTY");
}

#[test]
fn write_rolls_back_on_error() {
    let db = memory_db();
    let r = db.write(|c| {
        crate::domain::setup::upsert_department(c, &dept("바둑", &["바둑"], &["바둑"]))?;
        Err::<(), _>(crate::error::AppError::new("TEST", "일부러 실패"))
    });
    assert!(r.is_err());
    let list = db.read(|c| crate::domain::setup::list_departments(c)).unwrap();
    assert!(list.is_empty(), "실패하면 통째로 되돌아가야 한다");
}
