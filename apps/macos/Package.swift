// swift-tools-version: 6.2

import PackageDescription

let package = Package(
    name: "GhostNative",
    platforms: [
        .macOS(.v26)
    ],
    products: [
        .executable(name: "GhostNative", targets: ["GhostNative"])
    ],
    targets: [
        .executableTarget(
            name: "GhostNative",
            path: "Ghost"
        )
    ]
)
