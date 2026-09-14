//! P4-6 — **로그에 개인정보·업무 자료를 남기지 않는다.**
//!
//! 이 프로그램이 다루는 것은 학교 회계 자료다. 견적서에는 거래처의 사업자등록번호·전화번호·주소가,
//! 정산자료에는 부서별 금액이 들어 있다. 진단용 로그 한 줄 때문에 이런 것이 파일로 남으면 안 된다.
//!
//! **정책**
//!   1. 로그에는 *무슨 일이 있었는지*만 적는다. 자료의 *내용*은 적지 않는다.
//!   2. 파일 경로도 남기지 않는다 (경로에 학교·사람 이름이 들어 있는 일이 흔하다).
//!   3. 금액·품목명·상호·연락처·OCR 원문은 어떤 수준(`info`·`warn`·`error`)에서도 적지 않는다.
//!   4. 자세한 원인은 `AppError::detail` 에 담아 **화면 쪽으로만** 보낸다. 파일로 쓰지 않는다.
//!   5. `println!`·`dbg!` 는 제품 코드에 두지 않는다.
//!
//! 아래 시험이 이 정책을 코드에서 직접 확인한다. 새 로그를 넣으면 여기서 걸린다.

#[cfg(test)]
mod tests {
    use std::path::{Path, PathBuf};

    /// 로그 인자에 들어가면 안 되는 이름들 — 자료의 내용이나 자리를 가리킨다
    const FORBIDDEN: &[&str] = &[
        // 사람·거래처
        "display_name",
        "raw_name",
        "vendor_name",
        "mgmt_name",
        "phrase",
        "vendor_name_in_doc",
        // 파일 자리
        "source_path",
        "source_name",
        "to_string_lossy",
        "display()",
        "dest_dir",
        "out_path",
        "in_path",
        "file_name",
        // 금액
        "amount",
        "item_sum",
        "grand_total",
        "supply_total",
        "computed_total",
        "compare_total",
        "unit_price",
        // 읽은 글자
        "raw_grand_total",
        "loose_text",
        "blocks",
        "glyphs",
        ".text",
    ];

    fn rust_files(dir: &Path, out: &mut Vec<PathBuf>) {
        let Ok(entries) = std::fs::read_dir(dir) else { return };
        for e in entries.flatten() {
            let p = e.path();
            if p.is_dir() {
                rust_files(&p, out);
            } else if p.extension().is_some_and(|x| x == "rs") {
                out.push(p);
            }
        }
    }

    /// 시험 코드는 제외한다 (시험은 화면에 값을 찍어 봐야 한다)
    fn is_product_code(path: &Path) -> bool {
        let name = path.file_name().unwrap_or_default().to_string_lossy().to_string();
        !name.ends_with("_tests.rs")
    }

    /// `log::info!(...)` 같은 호출 한 줄에서 괄호 안을 꺼낸다
    fn log_calls(src: &str) -> Vec<String> {
        let mut out = Vec::new();
        for level in ["info", "warn", "error", "debug", "trace"] {
            let needle = format!("log::{level}!(");
            let mut from = 0;
            while let Some(i) = src[from..].find(&needle) {
                let start = from + i + needle.len();
                // 괄호 짝을 맞춰 끝을 찾는다
                let mut depth = 1;
                let mut end = start;
                for (k, c) in src[start..].char_indices() {
                    match c {
                        '(' => depth += 1,
                        ')' => {
                            depth -= 1;
                            if depth == 0 {
                                end = start + k;
                                break;
                            }
                        }
                        _ => {}
                    }
                }
                out.push(src[start..end].to_string());
                from = end.max(start + 1);
            }
        }
        out
    }

    fn sources() -> Vec<(PathBuf, String)> {
        let mut files = Vec::new();
        rust_files(Path::new(env!("CARGO_MANIFEST_DIR")).join("src").as_path(), &mut files);
        files
            .into_iter()
            .filter(|p| is_product_code(p))
            .filter_map(|p| std::fs::read_to_string(&p).ok().map(|s| (p, s)))
            .collect()
    }

    /// 제품 코드의 로그에 자료 내용이 들어가지 않는다
    #[test]
    fn logs_never_carry_business_data() {
        let mut seen = 0;
        for (path, src) in sources() {
            // 이 파일 자체는 금지어 목록을 담고 있으므로 건너뛴다
            if path.ends_with("logpolicy.rs") {
                continue;
            }
            for call in log_calls(&src) {
                seen += 1;
                for bad in FORBIDDEN {
                    assert!(
                        !call.contains(bad),
                        "{}: 로그에 '{bad}' 가 들어간다 → {call}",
                        path.display()
                    );
                }
            }
        }
        assert!(seen > 0, "로그 호출을 하나도 못 찾았다 — 시험이 헛돌고 있다");
        println!("로그 호출 {seen}건 확인");
    }

    /// 제품 코드에는 `println!`·`dbg!` 를 두지 않는다 (콘솔에 자료가 새어 나간다)
    #[test]
    fn no_debug_printing_in_product_code() {
        for (path, src) in sources() {
            if path.ends_with("logpolicy.rs") {
                continue;
            }
            // 시험 모듈(`mod tests`) 아래는 본다고 쳐도 상관없으므로 그 앞까지만 본다
            let head = match src.find("\n#[cfg(test)]") {
                Some(i) => &src[..i],
                None => &src[..],
            };
            for bad in ["println!(", "eprintln!(", "dbg!("] {
                assert!(!head.contains(bad), "{}: 제품 코드에 {bad} 가 있다", path.display());
            }
        }
    }

    /// 자세한 원인은 `detail` 로만 간다 — 파일로 쓰는 자리가 없어야 한다
    #[test]
    fn nothing_writes_a_log_file() {
        for (path, src) in sources() {
            if path.ends_with("logpolicy.rs") {
                continue;
            }
            for bad in ["tauri_plugin_log", "env_logger", "simplelog", "File::create(\"log"] {
                assert!(
                    !src.contains(bad),
                    "{}: 로그 파일을 쓰는 것으로 보인다 ({bad}). \
                     넣으려면 먼저 로그 정책(P4-6)을 다시 보라",
                    path.display()
                );
            }
        }
    }
}
