//! 좌표 → 표 복원 시험. P0-6 에서 주산암산 JPG 로 실측한 좌표를 그대로 쓴다.

use super::*;

fn b(text: &str, x0: f64, x1: f64, y0: f64, h: f64) -> Block {
    Block { text: text.into(), x0, x1, y0, y1: y0 + h, origin: "1쪽".into() }
}

/// P0-6 에서 실제로 얻은 주산암산 견적서(3배 확대) 좌표
fn jusan_blocks() -> Vec<Block> {
    vec![
        // 머리글 (세액이 'OH', 비고가 '비'/'고' 로 깨진 것까지 그대로)
        b("순번", 202.0, 267.0, 832.0, 35.0),
        b("품 명/규 격", 557.0, 732.0, 832.0, 35.0),
        b("단위", 1027.0, 1092.0, 832.0, 35.0),
        b("수량", 1162.0, 1233.0, 832.0, 35.0),
        b("단가", 1321.0, 1392.0, 832.0, 35.0),
        b("공급가액", 1540.0, 1677.0, 832.0, 35.0),
        b("OH", 1942.0, 1971.0, 832.0, 35.0),
        b("비", 2077.0, 2106.0, 832.0, 35.0),
        b("고", 2173.0, 2205.0, 832.0, 35.0),
        // 품목 1
        b("1", 229.0, 237.0, 916.0, 35.0),
        b("방과후 기초Yap! 상", 307.0, 624.0, 916.0, 35.0),
        b("권", 1045.0, 1074.0, 916.0, 35.0),
        b("3", 1237.0, 1254.0, 916.0, 35.0),
        b("10,000", 1324.0, 1434.0, 916.0, 35.0),
        b("30,000", 1651.0, 1764.0, 916.0, 35.0),
        // 품목 2
        b("2", 226.0, 243.0, 988.0, 35.0),
        b("방과후 기초Yap! 하", 307.0, 624.0, 988.0, 35.0),
        b("권", 1045.0, 1074.0, 988.0, 35.0),
        b("1", 1240.0, 1248.0, 988.0, 35.0),
        b("10,000", 1324.0, 1434.0, 988.0, 35.0),
        b("10,000", 1654.0, 1764.0, 988.0, 35.0),
        // 품목 3 — 품명이 짧다 (머리글 위치로 열을 나누면 여기서 깨진다)
        b("3", 226.0, 243.0, 1063.0, 35.0),
        b("10급Yap!", 310.0, 456.0, 1063.0, 35.0),
        b("권", 1045.0, 1074.0, 1063.0, 35.0),
        b("2", 1237.0, 1254.0, 1063.0, 35.0),
        b("10,000", 1324.0, 1434.0, 1063.0, 35.0),
        b("20,000", 1651.0, 1764.0, 1063.0, 35.0),
        // 품목 4 — 더 짧다
        b("4", 226.0, 243.0, 1135.0, 35.0),
        b("암산교재", 307.0, 444.0, 1135.0, 35.0),
        b("권", 1045.0, 1074.0, 1135.0, 35.0),
        b("1", 1240.0, 1248.0, 1135.0, 35.0),
        b("8,000", 1342.0, 1434.0, 1135.0, 35.0),
        b("8,000", 1672.0, 1764.0, 1135.0, 35.0),
    ]
}

#[test]
fn rebuilds_jusan_table_from_coordinates() {
    let table = to_raw_table(jusan_blocks(), "견적서", 24.0).expect("표 복원");

    // 머리글이 첫 줄
    assert_eq!(table.get(0, 0), "순번");
    assert_eq!(table.get(0, 1), "품 명/규 격");
    assert_eq!(table.get(0, 2), "단위");
    assert_eq!(table.get(0, 3), "수량");
    assert_eq!(table.get(0, 4), "단가");
    assert_eq!(table.get(0, 5), "공급가액");

    // 짧은 품명이 앞 열로 끌려가지 않아야 한다 (P0-6 에서 두 번 깨진 곳)
    assert_eq!(table.get(1, 1), "방과후 기초Yap! 상");
    assert_eq!(table.get(3, 1), "10급Yap!");
    assert_eq!(table.get(4, 1), "암산교재");
    assert_eq!(table.get(3, 0), "3", "순번은 순번 열에 남아야 한다");
    assert_eq!(table.get(4, 0), "4");
}

/// 복원한 표를 **공통 파서**에 넣어도 P1 과 같은 결과가 나와야 한다
#[test]
fn common_parser_gives_same_result_as_p0() {
    let raw = to_raw_table(jusan_blocks(), "견적서", 24.0).unwrap();
    let q = table::interpret(&raw);

    let items: Vec<_> = q.items.iter().filter(|i| i.kind == crate::domain::RowKind::Item).collect();
    assert_eq!(items.len(), 4, "품목 4건");
    assert_eq!(items[0].display_name, "방과후 기초Yap! 상");
    assert_eq!(items[0].spec, "권");
    assert_eq!(items[0].qty, Some(3));
    assert_eq!(items[0].unit_price, Some(10_000));
    assert_eq!(items[0].amount, Some(30_000));
    assert_eq!(items[1].display_name, "방과후 기초Yap! 하");
    assert_eq!(items[2].display_name, "10급Yap!");
    assert_eq!(items[2].amount, Some(20_000));
    assert_eq!(items[3].display_name, "암산교재");
    assert_eq!(items[3].amount, Some(8_000));

    // 네 행 모두 수량×단가=금액
    for it in &items {
        assert!(it.warnings.is_empty(), "{} {:?}", it.display_name, it.warnings);
    }
    assert_eq!(q.item_sum, 68_000, "품목 합 = 정산자료 주산암산 68,000");

    // 문구도 사람이 쓴 것과 같아야 한다
    let rows: Vec<crate::domain::phrase::PhraseRow> = q
        .items
        .iter()
        .map(|i| crate::domain::phrase::PhraseRow {
            kind: i.kind,
            display_name: i.display_name.clone(),
        })
        .collect();
    assert_eq!(
        crate::domain::phrase::build("주산암산부", &rows, None).unwrap(),
        "주산암산부 방과후 기초Yap! 상 외 3종"
    );

    // 원본 위치가 남아 있어야 한다
    assert_eq!(items[0].cell_ref, "1쪽");
}

#[test]
fn rows_group_by_y() {
    let rows = group_rows(&jusan_blocks());
    // 머리글 1줄 + 품목 4줄
    assert_eq!(rows.len(), 5, "{:?}", rows.iter().map(|r| r.len()).collect::<Vec<_>>());
    assert_eq!(rows[0].len(), 9, "머리글 조각 9개");
}

#[test]
fn columns_come_from_body_not_header() {
    let rows = group_rows(&jusan_blocks());
    let body: Vec<&Vec<Block>> = rows.iter().skip(1).collect();
    let cols = cluster_columns(&body);
    assert_eq!(cols.len(), 6, "본문에서 6열이 나와야 한다: {cols:?}");
    // 품명 열은 넓고, 머리글 '품 명/규 격'(557~732)보다 왼쪽에서 시작한다
    assert!(cols[1].0 < 557.0, "품명 열이 머리글보다 왼쪽에서 시작: {:?}", cols[1]);
}

/// PDF 는 한 낱말이 여러 조각으로 쪼개져 나오는 일이 흔하다.
/// 얼마나 벌어져야 다른 칸으로 볼지는 `gap` 이 정한다.
#[test]
fn adjacent_pieces_merge() {
    let pieces = || {
        vec![
            b("방과", 100.0, 140.0, 10.0, 10.0),
            b("후", 141.0, 160.0, 10.0, 10.0),
            b("기초", 200.0, 240.0, 10.0, 10.0),
        ]
    };

    // 넉넉한 gap — 셋이 한 덩어리가 되고, 벌어진 자리에는 공백이 들어간다
    let wide = merge_adjacent(pieces(), 45.0);
    assert_eq!(wide.len(), 1);
    assert_eq!(wide[0].text, "방과후 기초");

    // 좁은 gap — 붙어 있는 둘만 합쳐지고 멀리 있는 것은 다른 칸으로 남는다
    let narrow = merge_adjacent(pieces(), 25.0);
    assert_eq!(narrow.len(), 2, "40픽셀 떨어진 조각은 다른 칸이다");
    assert_eq!(narrow[0].text, "방과후", "1픽셀 차이는 공백 없이 붙인다");
    assert_eq!(narrow[1].text, "기초");
}

#[test]
fn no_header_means_no_table() {
    let blocks = vec![
        b("아무", 10.0, 50.0, 10.0, 10.0),
        b("글자", 60.0, 100.0, 10.0, 10.0),
        b("여기", 10.0, 50.0, 30.0, 10.0),
    ];
    assert!(to_raw_table(blocks, "", 20.0).is_none());
}

#[test]
fn too_few_columns_means_no_table() {
    let blocks = vec![
        b("품명", 10.0, 50.0, 10.0, 10.0),
        b("금액", 100.0, 140.0, 10.0, 10.0),
        b("교재", 10.0, 50.0, 30.0, 10.0),
        b("1,000", 100.0, 140.0, 30.0, 10.0),
    ];
    // 열이 2개뿐이면 표로 보지 않는다 (잘못 읽느니 못 읽었다고 하는 편이 낫다)
    assert!(to_raw_table(blocks, "", 20.0).is_none());
}

/// 실제 OCR 은 같은 줄인데도 글자마다 높이가 조금씩 다르다.
/// y 중심만으로 줄세우면 오른쪽 끝 조각 뒤에 왼쪽 끝 조각이 와서
/// 떨어진 칸이 하나로 붙어 버린다 (실제 주산암산 JPG 에서 머리글이 통째로 깨졌다).
#[test]
fn pieces_far_apart_never_merge_even_when_y_differs() {
    let blocks = vec![
        // 오른쪽 끝, 살짝 낮은 글자 → y 중심 정렬에서 먼저 온다
        b("고", 724.7, 735.4, 277.7, 10.7),
        // 왼쪽 끝, 같은 줄
        b("순번", 67.7, 89.4, 277.7, 11.7),
        b("품 명/규 격", 185.7, 244.4, 277.7, 12.7),
    ];
    let merged = merge_adjacent(blocks, 6.0);
    let texts: Vec<&str> = merged.iter().map(|m| m.text.as_str()).collect();
    assert_eq!(texts, vec!["순번", "품 명/규 격", "고"], "떨어진 칸은 붙으면 안 된다");
}

/// 한글이 만든 PDF 의 머리글은 **칸 너비에 맞춰 벌려 쓴다**.
/// `품`·`명` 사이(46픽셀)가 `격`·`수` 사이(8픽셀)보다 넓어서 거리만으로는 못 나눈다.
/// 열 이름 사전으로 붙여야 `규격`·`수량`·`단가` 가 제자리를 찾는다.
/// (아래 좌표는 실제로 내보낸 `로봇과학 견적서.pdf` 에서 잰 값이다)
fn justified_header_blocks() -> Vec<Block> {
    let mut v = vec![
        b("품", 70.0, 79.9, 268.4, 10.0),
        b("명", 126.5, 136.4, 268.4, 10.0),
        b("규", 168.7, 178.7, 268.4, 10.0),
        b("격", 216.5, 226.4, 268.4, 10.0),
        b("수", 234.8, 244.8, 268.4, 10.0),
        b("량", 260.4, 270.4, 268.4, 10.0),
        b("단", 278.8, 288.7, 268.4, 10.0),
        b("가", 325.9, 335.9, 268.4, 10.0),
        b("공", 348.7, 358.7, 268.4, 10.0),
        b("급", 365.4, 375.4, 268.4, 10.0),
        b("가", 382.1, 392.0, 268.4, 10.0),
        b("액", 398.8, 408.7, 268.4, 10.0),
        b("세", 421.6, 431.5, 268.4, 10.0),
        b("액", 471.6, 481.6, 268.4, 10.0),
        b("비", 494.4, 504.4, 268.4, 10.0),
        b("고", 529.2, 539.2, 268.4, 10.0),
    ];
    // 본문 두 줄 (값은 실제 견적서 그대로)
    for (y, price, amount) in [(289.4, "76,500", "765,000"), (308.9, "13,500", "135,000")] {
        v.push(b("프로보테크닉 교구", 65.0, 158.5, y, 11.0));
        v.push(b("set", 190.0, 206.0, y, 11.0));
        v.push(b("10", 247.0, 260.2, y, 11.0));
        v.push(b(price, 291.0, 330.6, y, 11.0));
        v.push(b(amount, 364.0, 410.2, y, 11.0));
        v.push(b("부가세포함", 494.0, 544.0, y, 11.0));
    }
    v
}

#[test]
fn dictionary_rejoins_justified_header() {
    let mut row: Vec<Block> =
        justified_header_blocks().into_iter().filter(|x| x.y0 < 280.0).collect();
    row.sort_by(|p, q| p.x0.partial_cmp(&q.x0).unwrap());
    let joined = join_header_words(&row);
    let texts: Vec<&str> = joined.iter().map(|x| x.text.as_str()).collect();
    assert_eq!(texts, vec!["품명", "규격", "수량", "단가", "공급가액", "세액", "비고"]);
}

#[test]
fn justified_header_lands_on_the_right_columns() {
    let table = to_raw_table(justified_header_blocks(), "견적서", 9.0).expect("표 복원");
    assert_eq!(table.get(0, 0), "품명");
    assert_eq!(table.get(0, 1), "규격");
    assert_eq!(table.get(0, 2), "수량");
    assert_eq!(table.get(0, 3), "단가");
    assert_eq!(table.get(0, 4), "공급가액");

    let q = table::interpret(&table);
    let items: Vec<_> = q.items.iter().filter(|i| i.kind == crate::domain::RowKind::Item).collect();
    assert_eq!(items.len(), 2);
    assert_eq!(items[0].spec, "set");
    assert_eq!(items[0].qty, Some(10));
    assert_eq!(items[0].unit_price, Some(76_500));
    assert_eq!(items[0].amount, Some(765_000));
    assert_eq!(q.item_sum, 900_000);
}

/// 사전에 없는 글자는 **붙이지 않는다** — 없는 이름을 지어내지 않기 위해서다
#[test]
fn dictionary_join_never_invents_a_word() {
    let row = vec![
        b("품", 10.0, 20.0, 0.0, 10.0),
        b("절", 25.0, 35.0, 0.0, 10.0),
        b("수", 40.0, 50.0, 0.0, 10.0),
    ];
    let joined = join_header_words(&row);
    let texts: Vec<&str> = joined.iter().map(|x| x.text.as_str()).collect();
    assert_eq!(texts, vec!["품", "절", "수"], "품절수 같은 말은 만들지 않는다");
}

/// 아주 멀리 떨어진 조각까지 끌어와 붙이지는 않는다
#[test]
fn dictionary_join_respects_distance() {
    let row = vec![b("품", 10.0, 20.0, 0.0, 10.0), b("명", 400.0, 410.0, 0.0, 10.0)];
    let joined = join_header_words(&row);
    assert_eq!(joined.len(), 2, "멀리 떨어진 것은 다른 칸이다");
}
