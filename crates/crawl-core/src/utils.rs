pub fn to_err<E: std::fmt::Display>(error: E) -> String {
  error.to_string()
}
