# Push Notification Backend Requirements

## Why This Document Exists

The c9s iOS app implements client-side push notification handling (device token registration, notification categories, interactive actions, deep linking). However, **APNs push notifications require a server-side component** to deliver notifications to iOS devices. This document specifies what the Tervezo backend needs to implement.

## Problem

The iOS app currently uses two strategies for real-time updates:

1. **SSE streaming** (`/implementations/{id}/stream`) — works only when the app is in the foreground viewing a specific implementation
2. **Background polling** — the app polls `/implementations` every 30 seconds, but iOS restricts background execution aggressively (typically < 30 seconds of background time)

Neither approach can wake the app or alert the user when it's suspended or terminated. Push notifications via APNs solve this by delivering alerts through Apple's infrastructure regardless of app state.

### Key User Scenarios Requiring Push

| Scenario | Why Polling/SSE Fails |
|---|---|
| Implementation completes while phone is locked | App is suspended, no polling |
| Claude needs user input at 2 AM | App is terminated, no SSE connection |
| PR is auto-created after implementation | User has switched to another app |
| Implementation fails unexpectedly | User is not actively monitoring |

## Architecture

### Option A: SSE Relay Service (Recommended)

A lightweight relay service subscribes to SSE streams on behalf of registered iOS devices and forwards events as APNs notifications.

```
┌──────────────┐     SSE      ┌─────────────────────┐    APNs     ┌────────────┐
│ Tervezo API  │──────────────│  Notification Relay  │────────────│  iOS Device │
│ /stream      │  text/event- │  Service             │  HTTP/2    │  c9s app    │
│              │  stream      │                      │            │             │
└──────────────┘              │  - Subscribes to SSE │            └────────────┘
                              │  - Filters events    │
                              │  - Sends APNs push   │
                              │  - Manages tokens    │
                              └─────────────────────┘
```

**Pros:**
- No changes to the existing Tervezo API
- Relay can be deployed independently
- Simple to reason about: one SSE connection per registered user, filtered to relevant events

**Cons:**
- Relay must maintain persistent SSE connections (one per user with active implementations)
- Relay needs its own APNs credentials (Apple Developer Program certificate or key)

### Option B: Tervezo API Webhook/Push Integration

The Tervezo API directly integrates with APNs and manages device tokens.

```
┌──────────────┐                    APNs     ┌────────────┐
│ Tervezo API  │─────────────────────────────│  iOS Device │
│              │    HTTP/2 push               │  c9s app    │
│  (internal   │                              │             │
│   event bus) │                              └────────────┘
└──────────────┘
```

**Pros:**
- Single system, no relay service to maintain
- Direct access to implementation state changes (no SSE parsing needed)
- Lower latency (no intermediary)

**Cons:**
- Requires modifying the Tervezo API codebase
- Tighter coupling between API and iOS-specific push infrastructure
- Backend team must manage APNs credentials

### Recommendation

**Option A** for initial deployment — a standalone relay service is faster to build, deploy, and iterate on without requiring changes to the core Tervezo API. Transition to **Option B** once push notifications are validated and the feature is stable.

## API Endpoints Required

The iOS app needs the following new endpoints for device token management. These can live in the relay service or the Tervezo API.

### `POST /devices`

Register an iOS device for push notifications.

**Request:**
```json
{
  "deviceToken": "a1b2c3d4e5f6...",
  "platform": "ios",
  "appVersion": "1.0.0",
  "osVersion": "18.3"
}
```

**Response:**
```json
{
  "deviceId": "dev_abc123",
  "registered": true
}
```

**Headers:**
- `Authorization: Bearer tzv_...` (same Tervezo API key)

**Notes:**
- The `deviceToken` is the hex-encoded APNs token (see `NotificationService.didRegisterForRemoteNotifications` in the iOS app)
- Multiple devices can be registered per API key (user may have iPhone + iPad)
- Tokens should be deduplicated by value (re-registration updates metadata)

### `DELETE /devices/{deviceId}`

Unregister a device (e.g., on sign-out or app deletion).

**Response:**
```json
{
  "deleted": true
}
```

### `PUT /devices/{deviceId}/preferences`

Update notification preferences for a device.

**Request:**
```json
{
  "enabledCategories": [
    "IMPL_COMPLETED",
    "IMPL_FAILED",
    "IMPL_WAITING_INPUT",
    "IMPL_PR_READY"
  ],
  "quietHoursStart": null,
  "quietHoursEnd": null
}
```

**Response:**
```json
{
  "updated": true
}
```

**Notes:**
- Categories map to iOS notification categories defined in `NotificationService.Category`
- Quiet hours are optional; when set, suppress non-urgent notifications during the window

## APNs Payload Format

The iOS app's `NotificationService.parseNotificationPayload` expects the following structure in the APNs payload `userInfo` dictionary.

### Standard Payload Structure

```json
{
  "aps": {
    "alert": {
      "title": "Implementation Completed",
      "subtitle": "my-workspace/my-repo",
      "body": "feat: add user authentication — completed successfully"
    },
    "badge": 1,
    "sound": "default",
    "category": "IMPL_COMPLETED",
    "thread-id": "impl-abc123",
    "mutable-content": 1
  },
  "implementationId": "impl-abc123",
  "category": "IMPL_COMPLETED",
  "title": "feat: add user authentication",
  "message": "Implementation completed successfully"
}
```

### Fields

| Field | Location | Required | Description |
|---|---|---|---|
| `aps.alert.title` | APNs standard | Yes | Human-readable notification title |
| `aps.alert.subtitle` | APNs standard | No | Workspace/repo context |
| `aps.alert.body` | APNs standard | Yes | Implementation title + status message |
| `aps.badge` | APNs standard | No | Badge count (increment for unread) |
| `aps.sound` | APNs standard | Yes | Use `"default"` |
| `aps.category` | APNs standard | Yes | Must match iOS registered category |
| `aps.thread-id` | APNs standard | Yes | Group by implementation ID |
| `aps.mutable-content` | APNs standard | No | Set to `1` for notification content extension (Phase 10.5) |
| `implementationId` | Custom | Yes | Used for deep linking to implementation detail |
| `category` | Custom | Yes | Redundant for custom parsing (matches `aps.category`) |
| `title` | Custom | No | Implementation title for custom UI |
| `message` | Custom | No | Detailed status message |

### Payload Examples by Category

#### `IMPL_COMPLETED` — Implementation finished successfully

```json
{
  "aps": {
    "alert": {
      "title": "Implementation Completed",
      "subtitle": "acme-corp/web-app",
      "body": "feat: add dark mode toggle"
    },
    "sound": "default",
    "category": "IMPL_COMPLETED",
    "thread-id": "impl-abc123"
  },
  "implementationId": "impl-abc123",
  "category": "IMPL_COMPLETED",
  "title": "feat: add dark mode toggle",
  "message": "Completed in 12m 34s. 5 files changed (+142, -28)."
}
```

Interactive actions: **View Details**

#### `IMPL_FAILED` — Implementation encountered an error

```json
{
  "aps": {
    "alert": {
      "title": "Implementation Failed",
      "subtitle": "acme-corp/web-app",
      "body": "fix: resolve login bug — test suite failed"
    },
    "sound": "default",
    "category": "IMPL_FAILED",
    "thread-id": "impl-def456"
  },
  "implementationId": "impl-def456",
  "category": "IMPL_FAILED",
  "title": "fix: resolve login bug",
  "message": "Failed at step 'Run Tests': 3 tests failing"
}
```

Interactive actions: **View Details**, **Restart**

#### `IMPL_WAITING_INPUT` — Claude needs user input

```json
{
  "aps": {
    "alert": {
      "title": "Input Required",
      "subtitle": "acme-corp/web-app",
      "body": "feat: add OAuth login — Claude is waiting for your response"
    },
    "sound": "default",
    "category": "IMPL_WAITING_INPUT",
    "thread-id": "impl-ghi789"
  },
  "implementationId": "impl-ghi789",
  "category": "IMPL_WAITING_INPUT",
  "title": "feat: add OAuth login",
  "message": "Waiting for input at step 'Implementation'"
}
```

Interactive actions: **View Details**, **Respond**

This is the highest-priority notification — the user's response unblocks the implementation.

#### `IMPL_PR_READY` — Pull request was created

```json
{
  "aps": {
    "alert": {
      "title": "PR Created",
      "subtitle": "acme-corp/web-app #42",
      "body": "feat: add dark mode toggle — ready for review"
    },
    "sound": "default",
    "category": "IMPL_PR_READY",
    "thread-id": "impl-abc123"
  },
  "implementationId": "impl-abc123",
  "category": "IMPL_PR_READY",
  "title": "feat: add dark mode toggle",
  "message": "PR #42 created on branch feat/dark-mode"
}
```

Interactive actions: **View Details**, **Open PR**

## SSE Events to Push Notification Mapping

The relay service must map SSE events from `/implementations/{id}/stream` to push notifications.

| SSE Event | Push Category | Trigger Condition |
|---|---|---|
| `complete` with `success: true` | `IMPL_COMPLETED` | Always |
| `error` or `complete` with `error` | `IMPL_FAILED` | Always |
| `waiting_for_input` | `IMPL_WAITING_INPUT` | Always (high priority) |
| Timeline message with PR creation | `IMPL_PR_READY` | When `prUrl` first appears |
| `status` change to `stopped`/`cancelled` | `IMPL_FAILED` | Always |

### Deduplication

- Do not send duplicate notifications for the same event
- Use `thread-id` (implementation ID) to group related notifications
- If an implementation completes and a PR was already notified, do not re-notify about PR
- Track last notification sent per implementation + category to avoid repeats

### Rate Limiting

- Maximum 1 notification per implementation per category per 5 minutes
- Exception: `IMPL_WAITING_INPUT` has no rate limit (user input is time-sensitive)
- Batch multiple completions if a user has many implementations finishing simultaneously

## Relay Service Implementation Notes

### Technology Recommendations

- **Language:** Rust (matches c9s codebase), Node.js, or Go
- **APNs library:** `a2` (Rust), `@parse/node-apn` (Node.js), `sideshow/apns2` (Go)
- **Storage:** PostgreSQL or SQLite for device tokens and notification state
- **Deployment:** Single container, stateless except for SSE connections (store tokens in DB)

### Connection Management

1. On device registration (`POST /devices`), start an SSE connection to each of the user's active implementations
2. On device unregistration, close SSE connections if no other devices need them
3. Periodically poll `/implementations?status=running` to discover new implementations that need SSE subscriptions
4. Close SSE connections for completed/failed implementations after sending the final notification

### APNs Configuration

- **APNs Auth Key** (`.p8` file) — preferred over certificate-based auth
- **Team ID** and **Key ID** from Apple Developer Portal
- **Bundle ID:** `com.waxb.c9s` (or as configured in the iOS project)
- **Environment:** Use `api.sandbox.push.apple.com` for development, `api.push.apple.com` for production

### Error Handling

- **APNs 410 (Unregistered):** Remove the device token from the database
- **APNs 429 (Too Many Requests):** Back off and retry with exponential delay
- **SSE disconnection:** Reconnect with the same backoff logic as the iOS app (1s to 30s exponential)
- **Invalid device token format:** Log and skip (do not retry)

## iOS App Changes Required

The iOS app (`NotificationService.swift`) already handles:
- APNs registration and device token capture
- Notification categories and interactive actions
- Payload parsing and deep link routing

The following changes are needed to integrate with the backend:

1. **Send device token to backend** — After receiving the token in `didRegisterForRemoteNotifications`, call `POST /devices` to register:

```swift
// In NotificationService.didRegisterForRemoteNotifications:
func didRegisterForRemoteNotifications(deviceToken token: Data) {
    let tokenString = token.map { String(format: "%02x", $0) }.joined()
    self.deviceToken = tokenString

    // NEW: Register with backend
    Task {
        try? await TervezoService().registerDevice(token: tokenString)
    }
}
```

2. **Unregister on sign-out** — Call `DELETE /devices/{deviceId}` when the user signs out or clears their API key.

3. **Preferences UI** — Add notification preference controls in `SettingsView` to let users choose which categories to receive (calls `PUT /devices/{deviceId}/preferences`).

## Security Considerations

- Device tokens are opaque to the relay service — they are passed through to APNs
- The relay service must authenticate with the Tervezo API using the user's API key (stored encrypted)
- APNs auth keys (`.p8`) must never be exposed in client code or version control
- Device registration endpoint must validate the bearer token before accepting a device token
- Rate limit the `POST /devices` endpoint to prevent abuse (e.g., 10 registrations per hour per API key)

## Testing

### Relay Service Tests

1. Register a device token, trigger an SSE event, verify APNs payload is sent
2. Unregister device, trigger event, verify no APNs call
3. Simulate APNs 410 response, verify device token is cleaned up
4. Test deduplication: send same event twice, verify only one notification
5. Test rate limiting: rapid-fire events, verify throttling works

### iOS Integration Tests

1. Receive a push notification with `IMPL_COMPLETED` payload, verify deep link to detail view
2. Tap "Restart" action on `IMPL_FAILED` notification, verify restart API call
3. Tap "Respond" on `IMPL_WAITING_INPUT`, verify navigation to detail with prompt input focused
4. Test badge count increment/decrement

### End-to-End Test

1. Create implementation via API
2. Register iOS device for push
3. Let implementation run to completion
4. Verify push notification received on device
5. Tap notification, verify app opens to correct implementation detail

## Timeline

| Phase | Effort | Dependency |
|---|---|---|
| Device registration API (`POST/DELETE /devices`) | 1-2 days | None |
| SSE relay service (core loop) | 3-5 days | Device registration API |
| APNs integration | 1-2 days | Apple Developer Program membership |
| Notification preferences API | 1 day | Device registration API |
| iOS app token registration integration | 0.5 days | Device registration API deployed |
| iOS settings UI for preferences | 0.5 days | Preferences API deployed |
| End-to-end testing | 1-2 days | All above |
| **Total** | **8-13 days** | |
