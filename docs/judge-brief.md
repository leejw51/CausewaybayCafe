# Causewaybay Cafe — Put Muse behind the counter

## Submission description

Busy neighborhood cafés should not need a complicated interface to use AI. Causewaybay Cafe turns the owner's Mac into a locally hosted café assistant: guests scan a table QR, order in natural language, and ask follow-up questions; staff receive the same order and prepare and serve it; owners monitor operations and AI usage.

Muse Spark interprets requests using the actual menu, conversation history and order context, then calls bounded backend tools. Rust validates permissions, quantities and prices before committing changes to SQLite. “What's my order?” is grounded in the recorded order, rather than a conversational guess.

Muse Image adds a visual order preview so guests can recognize their selection. Images are optional illustrations; exact items, quantities and prices remain authoritative. Caching and usage limits keep this feature from generating a new image on every message.

The distinction is a complete operational loop: the conversation changes café state, staff act on that state, and the guest sees the result. Business records live on the owner's Mac; Meta inference runs through the cloud API. This prototype uses demo wallet credits and manual payment records, not real payment processing.

## What to watch in the recorded demonstration

| Approximate time | Evidence |
| --- | --- |
| 0:00–0:32 | Guest request becomes an order with database-calculated quantities and total. |
| 0:32–0:53 | Follow-up conversation and optional Muse Image order illustration. |
| 0:53–1:14 | Staff acts on the same order through preparation and service. |
| 1:14–1:44 | Owner dashboard exposes hourly/daily usage and operational controls. |
| 1:44–3:00 | Management and closing guest view; demo fulfillment is disclosed. |

The captioned MP4 is `artifacts/demo/causewaybay-muse-demo-captioned.mp4`. It contains real Meta requests captured against disposable shop data. It has English captions and no narration. Artifacts are intentionally excluded from Git; upload the video separately to the submission's permitted destination.

## Why Meta is essential

- **Muse Spark:** converts conversational intent into validated operational tool calls; the menu and persisted state constrain what can be ordered.
- **Muse Image:** generates a recognizable visual accompaniment to an actual order, with caching and explicit usage controls.
- **Meta vision:** supports editable menu-photo and approximate floor-plan drafts. These are secondary prototype features, not the core recorded proof.

Guest speech currently uses browser recognition and then the same chat path. Do not present it as Meta speech recognition. The café mascot is existing OpenAI-generated artwork; the order illustration uses Meta.

## Evidence and limits

The latest recorded verification includes 40 Rust tests, 60 Playwright tests and 64 synthetic service/payment flows with eight concurrent clients. After the final receipt fix, all 40 Rust tests and 24 relevant browser tests passed again. Synthetic stress testing makes no paid AI requests and is separate from live Meta checks. See [verification](verification.md) for scope and history.

This is a single-shop prototype. It does not claim fully offline inference, real card settlement, measured business savings or production deployment. Physical microphone quality and real restaurant-photo accuracy still need device/pilot checks.

## Judge reproduction

Follow the [README](../README.md) for macOS dependencies and configuration. Set your own `MUSE_API_KEY` and public owner wallet address, then run `make start`. Use `make stresstest` for isolated, synthetic end-to-end verification. Never distribute an owner's API key, recovery phrase or café database.

## Before submitting

- Confirm acceptance, selected track and actual form requirements in the organizer portal.
- Upload the captioned MP4 and verify the resulting link from a signed-out browser, without Tailscale.
- Ensure judges can access the repository and the documented revision.
- Check the final video from start to finish; retain the raw recording and subtitles.
- Keep production/pilot claims limited to measured evidence.

The [official event page](https://dev.meta.ai/events/global-hackathon) lists November 4, 2026 at 9 PM ET as the submission deadline: November 5 at 11 AM KST. Verify the portal for changes. Its published themes make **Build for Your World** the natural fit; the participant's actual track must be confirmed separately.

## Closing line

Put Muse behind the counter. Let people focus on hospitality.
