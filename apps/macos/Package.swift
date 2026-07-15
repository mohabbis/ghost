// swift-tools-version: 5.9

import PackageDescription

let package = Package(
    name: "GhostNative",
    platforms: [
        .macOS(.v12)
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
