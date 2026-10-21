use crate::error::{ApiError, ApiResult};

pub const PASSWORD_MIN: usize = 8;
pub const PASSWORD_MAX: usize = 128;

pub fn password(value: &str) -> ApiResult<()> {
    let length = value.chars().count();
    if length < PASSWORD_MIN {
        return Err(ApiError::InvalidInput(format!(
            "Password must be at least {PASSWORD_MIN} characters"
        )));
    }
    if length > PASSWORD_MAX {
        return Err(ApiError::InvalidInput(format!(
            "Password must be at most {PASSWORD_MAX} characters"
        )));
    }
    if !value.chars().any(|c| c.is_ascii_digit()) {
        return Err(ApiError::InvalidInput("Password must contain at least one number".into()));
    }
    Ok(())
}

pub fn email(value: &str) -> ApiResult<String> {
    let trimmed = value.trim();
    let valid = trimmed.len() <= 254
        && !trimmed.contains(char::is_whitespace)
        && trimmed
            .split_once('@')
            .is_some_and(|(local, domain)| {
                !local.is_empty() && domain.contains('.') && !domain.starts_with('.') && !domain.ends_with('.')
            });
    if valid {
        Ok(trimmed.to_string())
    } else {
        Err(ApiError::InvalidInput("Enter a valid email address".into()))
    }
}

pub fn name(value: &str) -> ApiResult<String> {
    required(value, "Name", 100)
}

pub fn phone(value: &str) -> ApiResult<String> {
    let trimmed = value.trim();
    if trimmed.len() == 10 && trimmed.chars().all(|c| c.is_ascii_digit()) {
        Ok(trimmed.to_string())
    } else {
        Err(ApiError::InvalidInput("Enter a valid 10-digit mobile number".into()))
    }
}

pub fn required(value: &str, field: &str, max: usize) -> ApiResult<String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Err(ApiError::InvalidInput(format!("{field} is required")));
    }
    if trimmed.chars().count() > max {
        return Err(ApiError::InvalidInput(format!("{field} must be at most {max} characters")));
    }
    Ok(trimmed.to_string())
}
