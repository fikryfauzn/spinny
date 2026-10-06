pub(crate) fn relative_target(
    current: u64,
    duration: Option<u64>,
    delta: i64,
) -> Result<u64, &'static str> {
    if delta == 0 {
        return Err("scan delta must be nonzero");
    }
    if delta < 0 {
        return Ok(current.saturating_sub(delta.unsigned_abs()));
    }
    if let Some(end) = duration {
        return Ok(current.saturating_add(delta as u64).min(end.max(current)));
    }
    current
        .checked_add(delta as u64)
        .ok_or("scan target overflow")
}

#[cfg(test)]
mod tests {
    use super::relative_target;

    #[test]
    fn relative_targets_use_live_position_and_saturate_endpoints() {
        assert_eq!(relative_target(2_000, Some(10_000), 1_600), Ok(3_600));
        assert_eq!(relative_target(1_000, Some(10_000), -1_600), Ok(0));
        assert_eq!(relative_target(9_000, Some(10_000), 1_600), Ok(10_000));
        assert_eq!(relative_target(11_000, Some(10_000), 1_600), Ok(11_000));
        assert_eq!(relative_target(100, None, -1), Ok(99));
        assert_eq!(relative_target(100, None, i64::MIN), Ok(0));
        assert!(relative_target(0, None, 0).is_err());
        assert!(relative_target(u64::MAX, None, 1).is_err());
        assert_eq!(relative_target(u64::MAX, Some(u64::MAX), 1), Ok(u64::MAX));
    }
}
