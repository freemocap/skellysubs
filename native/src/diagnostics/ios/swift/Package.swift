// swift-tools-version:5.3
import PackageDescription

let package = Package(
    name: "tauri-plugin-diagnostic-sharing",
    platforms: [.iOS(.v13)],
    // Match the package/product name so swift-rs can identify this module's
    // C entry points when restoring their visibility on Xcode 27.
    products: [.library(name: "tauri-plugin-diagnostic-sharing", type: .static,
                        targets: ["tauri-plugin-diagnostic-sharing"])],
    dependencies: [.package(name: "Tauri", path: "../.tauri/tauri-api")],
    targets: [.target(name: "tauri-plugin-diagnostic-sharing", dependencies: [.byName(name: "Tauri")],
                      path: "Sources")]
)
