use std::path::{Path, PathBuf};
use std::sync::Arc;

use axum::{
    body::Bytes,
    extract::{Path as AxumPath, Query, State},
    http::header,
    response::IntoResponse,
    Json,
};
use serde::{Deserialize, Serialize};
use tokio::io::AsyncWriteExt;

use crate::{
    error::{AppError, AppResult},
    state::AppState,
};

#[derive(Debug, Deserialize)]
pub struct UploadAttachmentQuery {
    filename: String,
}

#[derive(Debug, Serialize)]
struct UploadedAttachment {
    path: String,
    name: String,
}

pub async fn post_attachment(
    Query(query): Query<UploadAttachmentQuery>,
    State(state): State<Arc<AppState>>,
    body: Bytes,
) -> AppResult<impl IntoResponse> {
    let name = safe_filename(&query.filename)?;
    let attachments_root = state.mosaic_root.join("attachments");
    tokio::fs::create_dir_all(&attachments_root).await?;

    let (name, mut file) = create_collision_safe_file(&attachments_root, &name).await?;
    file.write_all(&body).await?;
    file.flush().await?;

    Ok(Json(UploadedAttachment {
        path: format!("attachments/{name}"),
        name,
    }))
}

async fn create_collision_safe_file(
    attachments_root: &Path,
    original_name: &str,
) -> AppResult<(String, tokio::fs::File)> {
    for suffix in 0.. {
        let name = collision_name(original_name, suffix);
        let path = attachments_root.join(&name);
        match tokio::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(path)
            .await
        {
            Ok(file) => return Ok((name, file)),
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(error) => return Err(error.into()),
        }
    }
    unreachable!("collision suffix range is unbounded")
}

fn collision_name(original_name: &str, suffix: u32) -> String {
    if suffix == 0 {
        return original_name.to_string();
    }
    match original_name.rsplit_once('.') {
        Some((stem, extension)) if !stem.is_empty() => {
            format!("{stem}-{suffix}.{extension}")
        }
        _ => format!("{original_name}-{suffix}"),
    }
}

fn safe_filename(filename: &str) -> AppResult<String> {
    if filename.is_empty()
        || filename == "."
        || filename == ".."
        || filename.contains('/')
        || filename.contains('\\')
        || filename.contains('\0')
    {
        return Err(AppError::Validation(
            "Attachment filename must be a safe basename".to_string(),
        ));
    }
    Ok(filename.to_string())
}

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

    Ok(([(header::CONTENT_TYPE, content_type(&file))], bytes))
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
