# c9s Mobile (iOS)

iOS companion app for [c9s](https://github.com/waxb/c9s) — monitor and manage your Tervezo AI implementations from anywhere.

## Requirements

- iOS 18.0+
- Xcode 16+
- Swift 6
- Tervezo API key (`tzv_...`)

## Getting Started

1. Open `c9s-ios/` as a Swift Package in Xcode:
   ```bash
   cd c9s-ios
   open Package.swift
   ```

2. Build & Run targeting an iOS 18 simulator or device.

3. On first launch, enter your Tervezo API key to authenticate.

## Architecture

- **MVVM** with `@Observable` ViewModels and protocol-based services
- **SwiftUI** for all views, targeting iOS 18+ APIs
- **SwiftData** for offline caching
- **Swift Concurrency** (async/await, actors) throughout
- **SSE streaming** via native `URLSession.bytes(for:)` for real-time updates

### Project Structure

```
c9s-ios/
├── Sources/
│   ├── App/                    # App entry point, RootView
│   ├── Models/                 # SwiftData @Model types
│   ├── Services/               # API client, keychain, SSE, notifications
│   ├── ViewModels/             # @Observable VMs
│   ├── Views/                  # SwiftUI views
│   │   └── Components/         # Reusable UI components
│   └── GeneratedAPI/           # OpenAPI spec + generator config
├── Tests/
│   ├── ServicesTests/          # API & cache tests
│   ├── ViewModelTests/         # VM unit tests
│   └── Mocks/                  # Mock services & fixtures
└── ci/                         # CI workflow template
```

### Key Files

| File | Purpose |
|---|---|
| `Services/TervezoService.swift` | Full REST API client (19 operations) |
| `Services/SSEStreamService.swift` | SSE real-time streaming with auto-reconnect |
| `Services/KeychainService.swift` | Secure API key storage |
| `Services/NotificationService.swift` | Push notifications + deep linking |
| `ViewModels/ImplementationListVM.swift` | List with filter, search, sort, polling |
| `ViewModels/ImplementationDetailVM.swift` | Detail with SSE, tabs, actions |
| `Views/ImplementationDetailView.swift` | Full detail view with timeline, plan, changes |

## Features

- **Implementation List** — search, filter by status, sort, pull-to-refresh, auto-polling
- **Implementation Detail** — tabbed view (Timeline, Plan, Changes, Tests), step progress
- **Real-Time Updates** — SSE streaming with auto-reconnect and exponential backoff
- **Interactive Actions** — send prompts, create/merge/close PRs, restart implementations
- **Create Implementation** — form with workspace selection, mode picker, prompt editor
- **SSH Terminal** — sandbox terminal access via browser or SSH command copy
- **Push Notifications** — implementation status updates with interactive actions
- **Deep Linking** — `c9s://implementation/{id}` URL scheme

## Testing

```bash
cd c9s-ios
swift test
```

The project includes ~90 unit tests covering:
- API client (12 tests)
- Keychain service (11 tests)
- Cache service (10 tests)
- Settings VM (8 tests)
- Implementation list VM (17 tests)
- Implementation detail VM (14 tests)
- Create implementation VM (20 tests)
- Terminal VM (8 tests)

## CI

Copy `ci/ios-ci.yml` to `.github/workflows/ios-ci.yml` to enable CI.

The workflow runs on macOS 15 with Xcode 16 and includes:
- SPM dependency resolution + caching
- Build + test
- SwiftLint

## Dependencies

| Package | Purpose |
|---|---|
| [swift-openapi-generator](https://github.com/apple/swift-openapi-generator) | OpenAPI spec → Swift types |
| [swift-openapi-runtime](https://github.com/apple/swift-openapi-runtime) | OpenAPI runtime support |
| [swift-openapi-urlsession](https://github.com/apple/swift-openapi-urlsession) | URLSession transport |
| [SwiftTerm](https://github.com/migueldeicaza/SwiftTerm) | Terminal emulator view |
