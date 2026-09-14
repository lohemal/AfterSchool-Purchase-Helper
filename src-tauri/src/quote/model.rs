//! 견적서 파싱의 공통 자료 구조.
//!
//! **모든 형식은 `RawTable` 로 정규화된 뒤 하나의 `table.rs` 가 품목을 뽑는다.**
//! 새 형식을 붙이는 일 = `RawTable` 을 만드는 함수 하나를 더하는 일.

use serde::{Deserialize, Serialize};

use crate::domain::{CompareBasis, Confidence, RowKind, SignEffect, Warning};

/// 형식과 무관한 문자열 격자. 셀마다 원본 위치를 함께 들고 다닌다.
#[derive(Debug, Clone, Default)]
pub struct RawTable {
    /// 표 제목 (있으면). 견적서와 납품서를 가르는 데 쓴다.
    pub title: String,
    pub rows: Vec<Vec<RawCell>>,
    /// 표 밖에서 찾은 글자 (합계금액 칸 등)
    pub loose_text: Vec<RawCell>,
}

#[derive(Debug, Clone, Default)]
pub struct RawCell {
    pub text: String,
    /// 원본 위치를 사람이 읽는 꼴로. 예: `견적서!B12` / `표1 10행 0열`
    pub cell_ref: String,
}

impl RawCell {
    pub fn new(text: impl Into<String>, cell_ref: impl Into<String>) -> Self {
        Self { text: text.into(), cell_ref: cell_ref.into() }
    }
    pub fn plain(text: impl Into<String>) -> Self {
        Self { text: text.into(), cell_ref: String::new() }
    }
}

impl RawTable {
    pub fn width(&self) -> usize {
        self.rows.iter().map(|r| r.len()).max().unwrap_or(0)
    }
    pub fn get(&self, r: usize, c: usize) -> &str {
        self.rows.get(r).and_then(|row| row.get(c)).map(|x| x.text.as_str()).unwrap_or("")
    }
    pub fn cell(&self, r: usize, c: usize) -> Option<&RawCell> {
        self.rows.get(r).and_then(|row| row.get(c))
    }
}

/// 뽑아낸 한 행
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ParsedItem {
    pub kind: RowKind,
    pub sign_effect: Option<SignEffect>,
    /// 품의에 쓸 이름. 처음에는 `raw_name` 과 같다.
    pub display_name: String,
    pub spec: String,
    pub qty: Option<i64>,
    pub unit_price: Option<i64>,
    /// 원문 부호 그대로
    pub amount: Option<i64>,
    pub raw_name: String,
    pub raw_spec: String,
    pub raw_qty: String,
    pub raw_unit_price: String,
    pub raw_amount: String,
    pub cell_ref: String,
    pub confidence: Confidence,
    pub warnings: Vec<Warning>,
}

/// 견적서 한 장을 읽은 결과
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ParsedQuote {
    pub items: Vec<ParsedItem>,
    /// 견적서에 적힌 값 (없으면 None)
    pub supply_total: Option<i64>,
    pub tax_total: Option<i64>,
    pub grand_total: Option<i64>,
    /// 표기 합계를 읽어 온 **원문 그대로** (사진에서 글자가 깨졌는지 보는 근거. 비어 있을 수 있다)
    #[serde(default)]
    pub raw_grand_total: String,
    #[serde(default)]
    pub raw_supply_total: String,
    /// kind=Item 금액의 합
    pub item_sum: i64,
    /// kind=Adjustment 를 sign_effect 대로 적용한 합
    pub adjustment_sum: i64,
    /// item_sum + adjustment_sum
    pub computed_total: i64,
    pub compare_basis: CompareBasis,
    pub vendor_name_in_doc: String,
    /// 표를 무엇으로 골랐는지 (여러 표일 때)
    pub table_note: String,
    /// 어느 경로로 읽었는가 (structured | pdf_text | ocr)
    #[serde(default)]
    pub source: String,
    /// 한 줄 신뢰 상태 (ok | needs_check | uncertain | amount_failed)
    #[serde(default)]
    pub trust: String,
    pub warnings: Vec<Warning>,
}

impl ParsedQuote {
    /// 정산과 비교할 값
    pub fn compare_total(&self) -> i64 {
        match self.compare_basis {
            CompareBasis::Grand => self.grand_total.unwrap_or(self.computed_total),
            CompareBasis::Supply => self.supply_total.unwrap_or(self.computed_total),
            CompareBasis::ComputedTotal => self.computed_total,
        }
    }
}

/// 지원하는 파일 형식
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum QuoteFormat {
    Xlsx,
    Xls,
    Xlsm,
    Hwpx,
    Hwp,
    /// 텍스트 PDF 면 좌표로, 스캔본이면 그림을 꺼내 OCR 로 (P2·P3)
    Pdf,
    /// 사진 — Windows 내장 OCR (P3)
    Image,
    Unsupported,
}

impl QuoteFormat {
    pub fn key(self) -> &'static str {
        match self {
            QuoteFormat::Xlsx => "xlsx",
            QuoteFormat::Xls => "xls",
            QuoteFormat::Xlsm => "xlsm",
            QuoteFormat::Hwpx => "hwpx",
            QuoteFormat::Hwp => "hwp",
            QuoteFormat::Pdf => "pdf",
            QuoteFormat::Image => "image",
            QuoteFormat::Unsupported => "unsupported",
        }
    }

    pub fn from_path(path: &std::path::Path) -> QuoteFormat {
        let ext = path
            .extension()
            .map(|e| e.to_string_lossy().to_lowercase())
            .unwrap_or_default();
        match ext.as_str() {
            "xlsx" => QuoteFormat::Xlsx,
            "xls" => QuoteFormat::Xls,
            "xlsm" => QuoteFormat::Xlsm,
            "hwpx" => QuoteFormat::Hwpx,
            "hwp" => QuoteFormat::Hwp,
            "pdf" => QuoteFormat::Pdf,
            "jpg" | "jpeg" | "png" | "bmp" | "tif" | "tiff" => QuoteFormat::Image,
            _ => QuoteFormat::Unsupported,
        }
    }

    /// 읽어 볼 수 있는 형식인가
    pub fn supported(self) -> bool {
        self != QuoteFormat::Unsupported
    }

    /// 읽는 데 시간이 걸리는가 (사진은 한 장에 20초쯤 걸린다 — 화면이 멈춘 것처럼 보이면 안 된다)
    pub fn is_slow(self) -> bool {
        matches!(self, QuoteFormat::Image | QuoteFormat::Pdf)
    }

    /// 못 읽는 형식에 대한 안내 문장
    pub fn unsupported_message(self) -> &'static str {
        "이 형식의 견적서는 읽지 못합니다. 품목을 직접 입력해 주세요."
    }
}
