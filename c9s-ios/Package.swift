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
    dependencies: [
        // OpenAPI code generation (build plugin + runtime + transport)
        .package(url: "https://github.com/apple/swift-openapi-generator", from: "1.0.0"),
        .package(url: "https://github.com/apple/swift-openapi-runtime", from: "1.0.0"),
        .package(url: "https://github.com/apple/swift-openapi-urlsession", from: "1.0.0"),

        // Server-Sent Events (SSE) with AsyncSequence support
        .package(url: "https://github.com/mattt/EventSource", from: "1.3.0"),

        // Native terminal emulator for SSH sessions
        .package(url: "https://github.com/migueldeicaza/SwiftTerm", from: "1.6.0"),
    ],
    targets: [
        // Generated API client from Tervezo OpenAPI spec
        .target(
            name: "TervezoAPI",
            dependencies: [
                .product(name: "OpenAPIRuntime", package: "swift-openapi-runtime"),
                .product(name: "OpenAPIURLSession", package: "swift-openapi-urlsession"),
            ],
            path: "Sources/GeneratedAPI",
            plugins: [
                .plugin(name: "OpenAPIGenerator", package: "swift-openapi-generator"),
            ]
        ),

        // Main app library
        .target(
            name: "C9sLib",
            dependencies: [
                "TervezoAPI",
                .product(name: "OpenAPIRuntime", package: "swift-openapi-runtime"),
                .product(name: "OpenAPIURLSession", package: "swift-openapi-urlsession"),
                .product(name: "EventSource", package: "EventSource"),
                .product(name: "SwiftTerm", package: "SwiftTerm"),
            ],
            path: "Sources",
            exclude: ["GeneratedAPI", "NotificationContent"]
        ),

        // Unit and integration tests
        .testTarget(
            name: "C9sTests",
            dependencies: [
                "C9sLib",
                "TervezoAPI",
            ],
            path: "Tests"
        ),
    ]
)
