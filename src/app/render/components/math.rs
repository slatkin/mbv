#[expect(
    clippy::cast_precision_loss,
    reason = "integer ratio is converted to f64 for fractional UI geometry"
)]
pub(in crate::app) fn int_ratio(numer: i64, denom: i64) -> f64 {
    numer as f64 / denom as f64
}
