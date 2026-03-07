# Push Notification Backend Requirements

This document describes the backend infrastructure needed to deliver APNs push notifications to the c9s iOS app when Tervezo implementation events occur.

## Overview

The iOS app registers for push notifications and receives a device token from APNs. A backend relay service must:

1. Accept device token registrations from iOS clients
2. Subscribe to implementation events (via SSE or webhooks)
3. Forward relevant events as APNs push notifications

## Required API Endpoints

### `POST /api/v1/devices`

Register a device for push notifications.

```json
{
  "deviceToken": "a1b2c3d4e5f6...",
  "platform": "ios",
  "appVersion": "1.0.0"
}
```

Response: `201 Created`

### `DELETE /api/v1/devices/{deviceToken}`

Unregister a device (e.g., on sign-out).

Response: `204 No Content`

## Notification Categories

The iOS app registers four notification categories. The backend should set the `category` field in the APNs payload to match:

| Category ID | Trigger | Available Actions |
|---|---|---|
| `IMPL_COMPLETED` | Implementation finished successfully | View Details |
| `IMPL_FAILED` | Implementation encountered an error | View Details, Restart |
| `IMPL_WAITING_INPUT` | Implementation is waiting for user input | View Details, Respond |
| `IMPL_PR_READY` | Pull request was created | View Details, Open PR |

## APNs Payload Format

```json
{
  "aps": {
    "alert": {
      "title": "Implementation Completed",
      "body": "\"Fix login bug\" finished successfully"
    },
    "badge": 1,
    "sound": "default",
    "category": "IMPL_COMPLETED",
    "mutable-content": 1
  },
  "implementationId": "impl-abc123",
  "category": "IMPL_COMPLETED",
  "title": "Fix login bug",
  "message": "Implementation completed successfully"
}
```

### Required Fields

| Field | Type | Description |
|---|---|---|
| `aps.alert.title` | string | Notification title |
| `aps.alert.body` | string | Notification body text |
| `aps.category` | string | One of the category IDs above |
| `aps.mutable-content` | int | Set to 1 for rich notification support |
| `implementationId` | string | Implementation ID for deep linking |
| `category` | string | Duplicate of aps.category for app-level parsing |

### Optional Fields

| Field | Type | Description |
|---|---|---|
| `title` | string | Implementation title for rich preview |
| `message` | string | Detailed message for rich preview |

## Implementation Options

### Option A: SSE Relay Service (Recommended)

A lightweight service that:
1. Maintains a mapping of `userId → [deviceToken]`
2. For each registered user, subscribes to their active implementations' SSE streams
3. When an SSE event matches a notification trigger (status change to completed, failed, waiting_for_input, or PR created), sends an APNs push
4. Manages SSE connection lifecycle (reconnect on failure, clean up when implementation completes)

Advantages: Real-time, reuses existing SSE infrastructure, no API changes needed.

### Option B: Webhook Support

Add webhook configuration to the Tervezo API:
1. Users register a webhook URL for implementation events
2. The API calls the webhook on status changes
3. A relay service receives webhooks and forwards as APNs pushes

Advantages: Decoupled from SSE, standard webhook pattern.

### Option C: Server-Side Event Bus

If the Tervezo backend has an internal event bus (e.g., Redis pub/sub, Kafka):
1. A notification worker subscribes to implementation events
2. Filters events by registered device tokens
3. Sends APNs pushes directly

Advantages: Most efficient, no extra network hops.

## APNs Configuration

The relay service needs:
- Apple Developer account with push notification capability
- APNs authentication key (`.p8` file) or certificate
- Key ID, Team ID, and Bundle ID (`com.waxb.c9s`)
- Use APNs HTTP/2 provider API (`api.push.apple.com`)

## Badge Management

The app does not currently manage badge counts. The backend should:
- Increment the badge count for each notification
- Reset to 0 when the user opens the app (the app will call a `POST /api/v1/devices/{token}/badge-reset` endpoint)

## Priority

This is a Phase 2 feature. The app is fully functional without push notifications — it uses SSE streaming for real-time updates when in the foreground and configurable background polling.
