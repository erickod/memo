//! Id de task: vira nome de arquivo e valor de frontmatter, então é validado na borda.

/// `^[A-Za-z0-9][A-Za-z0-9._-]*$`, sem `..`, até 64 caracteres.
pub fn is_valid_id(id: &str) -> bool {
    let mut chars = id.chars();
    let first_ok = chars.next().is_some_and(|c| c.is_ascii_alphanumeric());
    first_ok
        && id.len() <= 64
        && !id.contains("..")
        && chars.all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-'))
}
