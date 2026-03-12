fn main() {
  println!("cargo:rustc-link-lib=dylib=html2md");
  if let Ok(path) = std::env::var("HTML2MD_LIB_PATH") {
    println!("cargo:rustc-link-search=native={path}");
  }
}
