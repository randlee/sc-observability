//! Stable process exit codes for the `sc-otel` command.

pub(crate) const EXIT_OK: u8 = 0;
pub(crate) const EXIT_INTERNAL: u8 = 1;
pub(crate) const EXIT_USAGE: u8 = 2;
/// Input or configuration was rejected before any export was attempted.
pub(crate) const EXIT_VALIDATION: u8 = 3;
/// The official exporter reported a failed export.
/// Use 7 to distinguish delivery failures from success and local CLI errors (0–3).
pub(crate) const EXIT_EXPORT: u8 = 7;

#[cfg(test)]
mod tests {
    use super::{EXIT_EXPORT, EXIT_INTERNAL, EXIT_OK, EXIT_USAGE, EXIT_VALIDATION};

    #[test]
    fn process_exit_code_registry_preserves_the_cli_contract() {
        assert_eq!(EXIT_OK, 0);
        assert_eq!(EXIT_INTERNAL, 1);
        assert_eq!(EXIT_USAGE, 2);
        assert_eq!(EXIT_VALIDATION, 3);
        assert_eq!(EXIT_EXPORT, 7);
    }
}
