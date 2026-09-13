# Trace Notifications

This context describes how tracing events become notifications delivered to Slack or Discord.

## Language

**Trace Event**:
An event observed from the tracing subscriber before filtering or presentation.
_Avoid_: Log message, webhook event

**Prepared Notification**:
A platform-neutral notification that has passed filtering and contains the normalized trace information needed for presentation.
_Avoid_: WebhookMessageInputs, formatted event

**Notification Policy**:
Platform-specific rules that decide presentation and whether a prepared notification requests a mention.
_Avoid_: Ping type, core notification rules

**Mention Target**:
The Discord audience deliberately requested for an error notification: everyone, here, or one role.
_Avoid_: Ping type, recipient string

**Platform Message**:
A Slack- or Discord-specific message produced from a prepared notification under a notification policy.
_Avoid_: Payload, webhook message

**Webhook Delivery**:
The attempt to send a platform message to its configured webhook destination.
_Avoid_: Worker message, HTTP post
