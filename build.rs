fn main() {
    println!("cargo:rustc-link-search=native=.");
    println!("cargo:rustc-link-lib=static=cpp");
    println!("cargo:rustc-link-lib=static=gumbo");
    println!("cargo:rustc-link-lib=dylib=stdc++");
}
