// Gives super-duper-worker.exe a Windows version resource, so Explorer's file properties and
// Verify-WindowsRelease.ps1 can read the product and version. winresource takes FileVersion and
// ProductVersion from the workspace version in Cargo.toml.
fn main() {
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("windows") {
        return;
    }
    let mut resource = winresource::WindowsResource::new();
    resource
        .set("ProductName", "Super Duper")
        .set("FileDescription", "Super Duper worker")
        .set("OriginalFilename", "super-duper-worker.exe")
        .set("InternalName", "super-duper-worker")
        .set("CompanyName", "Gary Gentry")
        .set(
            "LegalCopyright",
            "Copyright (c) 2026 Gary Gentry. MIT License.",
        );
    resource
        .compile()
        .expect("compile the worker's Windows version resource");
}
