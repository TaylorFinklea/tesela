use std::path::{Path, PathBuf};
use std::sync::Arc;

use axum::{
    extract::{Path as AxumPath, State},
    http::header,
    response::IntoResponse,
};

use crate::{
    error::{AppError, AppResult},
    state::AppState,
};

pub async fn get_attachment(
    AxumPath(path): AxumPath<String>,
    State(state): State<Arc<AppState>>,
) -> AppResult<impl IntoResponse> {
    let file = resolve_attachment_path(&state.mosaic_root, &path)?;
    let bytes = tokio::fs::read(&file).await.map_err(|error| {
        if error.kind() == std::io::ErrorKind::NotFound {
            AppError::NotFound(format!("Attachment not found: {path}"))
        } else {
            AppError::Internal(error.into())
        }
    })?;

    // Attachments are user-imported bytes served on the app origin: without
    // a response CSP, navigating to a malicious SVG would execute script
    // with same-origin API access. `sandbox`/`default-src 'none'` neutralizes
    // that while <img> embedding (which never runs SVG script) keeps working.
    Ok((
        [
            (header::CONTENT_TYPE, content_type(&file).to_string()),
            (
                header::CONTENT_SECURITY_POLICY,
                "default-src 'none'; sandbox".to_string(),
            ),
            (header::X_CONTENT_TYPE_OPTIONS, "nosniff".to_string()),
        ],
        bytes,
    ))
}

fn resolve_attachment_path(mosaic_root: &Path, relative_path: &str) -> AppResult<PathBuf> {
    let mosaic_root = mosaic_root
        .canonicalize()
        .map_err(|error| AppError::Internal(error.into()))?;
    let attachments_root = mosaic_root
        .join("attachments")
        .canonicalize()
        .map_err(|error| {
            if error.kind() == std::io::ErrorKind::NotFound {
                AppError::NotFound("Attachment not found".to_string())
            } else {
                AppError::Internal(error.into())
            }
        })?;

    if !attachments_root.starts_with(&mosaic_root) {
        return Err(AppError::Validation(
            "Attachment directory escapes the mosaic".to_string(),
        ));
    }

    let file = attachments_root
        .join(relative_path)
        .canonicalize()
        .map_err(|error| {
            if error.kind() == std::io::ErrorKind::NotFound {
                AppError::NotFound(format!("Attachment not found: {relative_path}"))
            } else {
                AppError::Internal(error.into())
            }
        })?;

    if !file.starts_with(&attachments_root) {
        return Err(AppError::Validation(
            "Attachment path escapes the mosaic".to_string(),
        ));
    }

    Ok(file)
}

fn content_type(path: &Path) -> &'static str {
    let extension = path
        .extension()
        .and_then(|extension| extension.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();

    match extension.as_str() {
        "avif" => "image/avif",
        "bmp" => "image/bmp",
        "gif" => "image/gif",
        "ico" => "image/x-icon",
        "jpeg" | "jpg" => "image/jpeg",
        "png" => "image/png",
        "svg" => "image/svg+xml",
        "webp" => "image/webp",
        "pdf" => "application/pdf",
        "json" => "application/json",
        "txt" | "md" => "text/plain; charset=utf-8",
        _ => "application/octet-stream",
    }
}

#[cfg(test)]
mod tests {
    use super::resolve_attachment_path;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn rejects_paths_that_canonicalize_outside_attachments() {
        let mosaic = TempDir::new().expect("temp mosaic");
        fs::create_dir(mosaic.path().join("attachments")).expect("attachments directory");
        fs::write(mosaic.path().join("outside.txt"), b"outside").expect("outside file");

        let result = resolve_attachment_path(mosaic.path(), "../outside.txt");

        assert!(result.is_err());
    }
}
