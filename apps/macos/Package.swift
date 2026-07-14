// swift-tools-version: 5.10

import PackageDescription

let package = Package(
    name: "GhostNative",
    platforms: [
        .macOS(.v13)
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
