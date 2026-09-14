//! 좌표에서 표를 되살리는 공통 로직 (P2 텍스트 PDF · P3 이미지 OCR 이 **함께** 쓴다).
//!
//! P0-6 에서 실측으로 정한 순서를 그대로 따른다.
//!
//!   1. y 로 행을 묶는다 (허용치 = 중앙 글자높이 × 0.6)
//!   2. 머리글 행을 찾는다 (낱말이 가장 많이 맞는 행)
//!   3. **본문 행의 x 구간을 겹침으로 군집화해 열을 만든다**
//!   4. 각 열 군집에 **가장 가까운 머리글 이름을 나중에 붙인다**
//!
//! ③ 과 ④ 의 방향이 반대라는 점이 중요하다 — **열은 자료가 정하고, 이름만 머리글에서 가져온다.**
//! 머리글 위치로 열을 나누면 왼쪽 정렬된 짧은 품명이 앞 열로 끌려간다(P0-6 에서 두 방식 모두 깨졌다).

use crate::quote::model::{RawCell, RawTable};
use crate::quote::table;

/// 글자 덩어리 하나 (PDF 의 한 줄 조각 · OCR 의 한 낱말/줄)
#[derive(Debug, Clone)]
pub struct Block {
    pub text: String,
    pub x0: f64,
    pub x1: f64,
    pub y0: f64,
    pub y1: f64,
    /// 원본 위치 (예: `1쪽`)
    pub origin: String,
}

impl Block {
    pub fn xc(&self) -> f64 {
        (self.x0 + self.x1) / 2.0
    }
    pub fn yc(&self) -> f64 {
        (self.y0 + self.y1) / 2.0
    }
    pub fn height(&self) -> f64 {
        (self.y1 - self.y0).abs()
    }
}

/// 한 덩어리로 볼 만큼 가까운 조각들을 미리 붙인다 (같은 줄에서 글자 사이 간격이 좁을 때).
///
/// **줄로 먼저 묶고, 그 줄 안에서 왼쪽부터** 붙인다.
/// y 중심만으로 줄세우면, 같은 줄인데 글자 높이가 1픽셀씩 다른 실제 OCR 조각들이
/// 뒤섞여 오른쪽 끝 글자 뒤에 왼쪽 끝 글자가 온다. 그러면 간격이 음수가 되어
/// 서로 멀리 떨어진 칸이 하나로 붙는다(실제 주산암산 JPG 에서 `고`+`순번` 이 붙었다).
pub fn merge_adjacent(blocks: Vec<Block>, gap: f64) -> Vec<Block> {
    if blocks.is_empty() {
        return blocks;
    }
    let mut out: Vec<Block> = Vec::new();
    for row in group_rows(&blocks) {
        // group_rows 가 줄 안을 x0 오름차순으로 정렬해 준다
        for b in row {
            match out.last_mut() {
                // 같은 줄 안에서, **오른쪽으로 이어질 때만** 붙인다
                Some(prev) if same_row(prev, &b) && b.x0 - prev.x1 <= gap && b.x1 >= prev.x1 => {
                    // 사이가 벌어져 있으면 공백을 하나 넣는다
                    if b.x0 - prev.x1 > gap * 0.35 && !prev.text.ends_with(' ') {
                        prev.text.push(' ');
                    }
                    prev.text.push_str(&b.text);
                    prev.x1 = prev.x1.max(b.x1);
                    prev.y0 = prev.y0.min(b.y0);
                    prev.y1 = prev.y1.max(b.y1);
                }
                _ => out.push(b),
            }
        }
    }
    out
}

/// 두 조각이 같은 줄인가
fn same_row(a: &Block, b: &Block) -> bool {
    (a.yc() - b.yc()).abs() <= a.height().max(b.height()).max(1.0) * 0.5
}

/// y 로 행을 묶는다.
pub fn group_rows(blocks: &[Block]) -> Vec<Vec<Block>> {
    if blocks.is_empty() {
        return Vec::new();
    }
    let mut sorted: Vec<Block> = blocks.to_vec();
    sorted.sort_by(|a, b| a.yc().partial_cmp(&b.yc()).unwrap_or(std::cmp::Ordering::Equal));

    let mut heights: Vec<f64> = sorted.iter().map(|b| b.height()).filter(|h| *h > 0.0).collect();
    heights.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let median = if heights.is_empty() { 10.0 } else { heights[heights.len() / 2] };
    let tol = (median * 0.6).max(1.0);

    let mut rows: Vec<Vec<Block>> = vec![vec![sorted[0].clone()]];
    for b in sorted.into_iter().skip(1) {
        let last_y = rows.last().unwrap().last().unwrap().yc();
        if (b.yc() - last_y).abs() <= tol {
            rows.last_mut().unwrap().push(b);
        } else {
            rows.push(vec![b]);
        }
    }
    for r in rows.iter_mut() {
        r.sort_by(|a, b| a.x0.partial_cmp(&b.x0).unwrap_or(std::cmp::Ordering::Equal));
    }
    rows
}

/// 벌려 쓴 머리글 조각을 **열 이름 사전에 맞는 만큼만** 도로 붙인다.
///
/// 한글이 만든 PDF 는 머리글을 칸 너비에 맞춰 벌려 쓴다.
/// 그래서 `품`·`명` 이 40픽셀 떨어진 딴 조각으로 나오고, 거리만으로는 나눌 수 없다
/// (같은 칸 안 간격이 옆 칸과의 간격보다 넓다).
/// 사전에 있는 낱말이 **정확히** 만들어질 때만 붙이므로 없는 이름을 지어내지 않는다.
pub fn join_header_words(row: &[Block]) -> Vec<Block> {
    let mut out: Vec<Block> = Vec::new();
    let mut i = 0usize;
    while i < row.len() {
        let mut text = row[i].text.clone();
        let mut best: Option<(usize, String)> = None;
        let mut j = i;
        while j + 1 < row.len() {
            // 너무 멀리 떨어진 조각까지 끌어오지 않는다
            let gap = row[j + 1].x0 - row[j].x1;
            let limit = row[j].height().max(row[j + 1].height()).max(1.0) * 6.0;
            if gap > limit {
                break;
            }
            if !table::is_role_prefix(&text) {
                break;
            }
            j += 1;
            text.push_str(&row[j].text);
            if table::role_of(&text).is_some() {
                best = Some((j, text.clone()));
            }
        }
        match best {
            Some((end, joined)) => {
                let mut b = row[i].clone();
                b.text = joined;
                b.x1 = row[end].x1;
                b.y0 = row[i..=end].iter().map(|x| x.y0).fold(f64::MAX, f64::min);
                b.y1 = row[i..=end].iter().map(|x| x.y1).fold(f64::MIN, f64::max);
                out.push(b);
                i = end + 1;
            }
            None => {
                out.push(row[i].clone());
                i += 1;
            }
        }
    }
    out
}

/// 머리글 행 = 열 이름 낱말이 가장 많이 맞는 행. 몇 개 맞았는지도 돌려준다.
///
/// 조각이 벌어져 나오는 PDF 를 위해 **사전으로 도로 붙여 본 뒤** 센다.
pub fn find_header_row(rows: &[Vec<Block>]) -> Option<(usize, usize)> {
    let mut best: Option<(usize, usize)> = None;
    for (i, r) in rows.iter().enumerate() {
        let hits = join_header_words(r).iter().filter(|b| table::role_of(&b.text).is_some()).count();
        if hits >= 2 && best.map(|(_, h)| hits > h).unwrap_or(true) {
            best = Some((i, hits));
        }
    }
    best
}

/// 본문 행 블록들의 x 구간을 겹침으로 묶어 열을 만든다.
pub fn cluster_columns(body: &[&Vec<Block>]) -> Vec<(f64, f64)> {
    let mut spans: Vec<(f64, f64)> =
        body.iter().flat_map(|r| r.iter().map(|b| (b.x0, b.x1))).collect();
    spans.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal));

    let mut out: Vec<(f64, f64)> = Vec::new();
    for (x0, x1) in spans {
        match out.last_mut() {
            Some(c) if x0 <= c.1 => c.1 = c.1.max(x1),
            _ => out.push((x0, x1)),
        }
    }
    out
}

/// 좌표 덩어리들을 `RawTable` 로 되살린다.
///
/// 머리글을 못 찾으면 `None` — 부르는 쪽이 "표를 못 찾음" 으로 다룬다.
pub fn to_raw_table(blocks: Vec<Block>, title: &str, merge_gap: f64) -> Option<RawTable> {
    let pieces = blocks.clone();
    let blocks = merge_adjacent(blocks, merge_gap);
    if blocks.is_empty() {
        return None;
    }
    let rows = group_rows(&blocks);
    let (header_idx, _) = find_header_row(&rows)?;

    // 본문 후보 = 머리글 아래에서 조각이 3개 이상인 행
    let body: Vec<&Vec<Block>> = rows.iter().skip(header_idx + 1).filter(|r| r.len() >= 3).collect();
    if body.is_empty() {
        return None;
    }

    let clusters = cluster_columns(&body);
    if clusters.len() < 3 {
        return None;
    }

    // 머리글 이름은 **붙이기 전 조각**에서 다시 만든다.
    // 벌려 쓴 머리글은 거리로 붙이면 옆 칸 글자끼리 붙는다(`격`+`수`).
    // 사전으로 붙이면 `규격`·`수량` 이 제대로 나온다.
    let (hy0, hy1) = (
        rows[header_idx].iter().map(|b| b.y0).fold(f64::MAX, f64::min),
        rows[header_idx].iter().map(|b| b.y1).fold(f64::MIN, f64::max),
    );
    let mut head_pieces: Vec<Block> =
        pieces.iter().filter(|b| b.yc() >= hy0 && b.yc() <= hy1).cloned().collect();
    head_pieces.sort_by(|a, b| a.x0.partial_cmp(&b.x0).unwrap_or(std::cmp::Ordering::Equal));
    let joined = join_header_words(&head_pieces);
    // 사전으로 붙인 것이 더 많이 맞을 때만 쓴다 (사진처럼 이미 칸 단위로 오는 경우는 그대로)
    let count = |v: &[Block]| v.iter().filter(|b| table::role_of(&b.text).is_some()).count();
    let fallback = join_header_words(&rows[header_idx]);
    let header = if count(&joined) >= count(&fallback) { &joined } else { &fallback };
    let names: Vec<String> = clusters
        .iter()
        .map(|(a, b)| {
            let c = (a + b) / 2.0;
            header
                .iter()
                .min_by(|p, q| {
                    (p.xc() - c)
                        .abs()
                        .partial_cmp(&(q.xc() - c).abs())
                        .unwrap_or(std::cmp::Ordering::Equal)
                })
                .map(|h| h.text.trim().to_string())
                .unwrap_or_default()
        })
        .collect();

    let which = |b: &Block| -> usize {
        let c = b.xc();
        clusters
            .iter()
            .enumerate()
            .min_by(|(_, p), (_, q)| {
                ((p.0 + p.1) / 2.0 - c)
                    .abs()
                    .partial_cmp(&(((q.0 + q.1) / 2.0 - c).abs()))
                    .unwrap_or(std::cmp::Ordering::Equal)
            })
            .map(|(i, _)| i)
            .unwrap_or(0)
    };

    // 머리글 줄 + 본문 줄을 격자로
    let mut grid: Vec<Vec<RawCell>> = Vec::new();
    grid.push(
        names
            .iter()
            .map(|n| RawCell::new(n.clone(), header.first().map(|b| b.origin.clone()).unwrap_or_default()))
            .collect(),
    );

    for r in rows.iter().skip(header_idx + 1) {
        let mut cells: Vec<RawCell> = (0..clusters.len())
            .map(|_| RawCell::new("", r.first().map(|b| b.origin.clone()).unwrap_or_default()))
            .collect();
        for b in r {
            let k = which(b);
            if !cells[k].text.is_empty() {
                cells[k].text.push(' ');
            }
            cells[k].text.push_str(b.text.trim());
            cells[k].cell_ref = b.origin.clone();
        }
        if cells.iter().any(|c| !c.text.trim().is_empty()) {
            grid.push(cells);
        }
    }

    // 머리글 위쪽 글자는 합계금액 칸을 찾는 데 쓴다
    let loose: Vec<RawCell> = rows
        .iter()
        .take(header_idx)
        .flatten()
        .map(|b| RawCell::new(b.text.clone(), b.origin.clone()))
        .collect();

    Some(RawTable { title: title.to_string(), rows: grid, loose_text: loose })
}

#[cfg(test)]
#[path = "layout_tests.rs"]
mod layout_tests;
