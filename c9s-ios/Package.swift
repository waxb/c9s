// swift-tools-version: 6.0
import PackageDescription

let package = Package(
    name: "c9s-ios",
    platforms: [
        .iOS(.v18),
    ],
    products: [
        .library(
            name: "C9sLib",
            targets: ["C9sLib"]
        ),
    ],
    targets: [
        .target(
            name: "C9sLib",
            path: "Sources"
        ),
        .testTarget(
            name: "C9sTests",
            dependencies: ["C9sLib"],
            path: "Tests"
        ),
    ]
)
