// swift-tools-version: 5.10

import PackageDescription

// Future UniFFI wiring is intentionally documented here without making it a
// build dependency yet. The current package must remain buildable before the
// Rust mobile core emits generated bindings and iOS archives.
//
// Expected future inputs, relative to this Package.swift:
// - Generated/DeepSeekMobileAgentCore.swift
// - Generated/deepseek_mobile_agent_coreFFI.modulemap
// - Artifacts/libdeepseek_mobile_agent_core.a
// - or Artifacts/DeepSeekMobileAgentCore.xcframework
//
// Keep those paths out of the mock executable target until a production bridge
// target links the generated Swift bindings against the Rust static library.
let generatedUniFFIDirectory = "Generated"
let rustArtifactDirectory = "Artifacts"

let package = Package(
    name: "DeepSeekMobileDemo",
    platforms: [
        .iOS(.v17)
    ],
    products: [
        .executable(
            name: "DeepSeekMobileDemo",
            targets: ["DeepSeekMobileDemo"]
        )
    ],
    targets: [
        .executableTarget(
            name: "DeepSeekMobileDemo",
            path: ".",
            exclude: [
                generatedUniFFIDirectory,
                rustArtifactDirectory,
                "README.md",
                "Tests"
            ]
        ),
        .testTarget(
            name: "DeepSeekMobileDemoTests",
            dependencies: ["DeepSeekMobileDemo"],
            path: "Tests",
            exclude: [
                "README.md"
            ]
        )
    ]
)
