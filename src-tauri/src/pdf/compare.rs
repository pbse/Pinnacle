use crate::pdf::convert::parse_pdf_layout_internal;
use std::collections::HashSet;
use tauri::AppHandle;

#[derive(serde::Serialize)]
pub struct ComparisonResult {
    pub added: Vec<String>,
    pub removed: Vec<String>,
    pub shared_count: usize,
}

pub async fn compare_pdfs_text_internal(
    app_handle: Option<&AppHandle>,
    path1: &str,
    path2: &str,
) -> Result<ComparisonResult, String> {
    let text1 = parse_pdf_layout_internal(app_handle, path1).await?;
    let text2 = parse_pdf_layout_internal(app_handle, path2).await?;

    let lines1: HashSet<&str> = text1
        .lines()
        .map(|l| l.trim())
        .filter(|l| !l.is_empty())
        .collect();
    let lines2: HashSet<&str> = text2
        .lines()
        .map(|l| l.trim())
        .filter(|l| !l.is_empty())
        .collect();

    let added: Vec<String> = lines2.difference(&lines1).map(|s| s.to_string()).collect();
    let removed: Vec<String> = lines1.difference(&lines2).map(|s| s.to_string()).collect();
    let shared_count = lines1.intersection(&lines2).count();

    Ok(ComparisonResult {
        added,
        removed,
        shared_count,
    })
}

#[tauri::command]
pub async fn compare_pdfs_text(
    app_handle: AppHandle,
    path1: String,
    path2: String,
) -> Result<ComparisonResult, String> {
    compare_pdfs_text_internal(Some(&app_handle), &path1, &path2).await
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pdf::test_utils::{create_minimal_pdf, setup_unique_paths, teardown_unique_paths};

    #[tokio::test]
    async fn test_compare_pdfs_success() {
        let (test_dir, output_dir) = setup_unique_paths("compare_success");
        let path1 = test_dir.join("v1.pdf");
        let path2 = test_dir.join("v2.pdf");

        create_minimal_pdf(path1.to_str().unwrap(), 1, "Original").unwrap();
        create_minimal_pdf(path2.to_str().unwrap(), 1, "Modified").unwrap();

        let result =
            compare_pdfs_text_internal(None, path1.to_str().unwrap(), path2.to_str().unwrap())
                .await
                .unwrap();

        // We expect "Modified-Page 1" to be in added and "Original-Page 1" to be in removed
        assert!(result.added.iter().any(|l| l.contains("Modified")));
        assert!(result.removed.iter().any(|l| l.contains("Original")));

        teardown_unique_paths(&test_dir, &output_dir);
    }
}
