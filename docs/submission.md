# Submission: Muse for neighborhood cafés and restaurants

## One sentence

Causewaybay Cafe installs a Muse-powered AI employee on a restaurant owner's Mac: guests order naturally, staff fulfill orders, and owners manage the shop through one conversation-first system.

## Problem and differentiator

Small restaurants cannot stop service to operate complicated software. Guests may not know the menu or speak the local language. Instead of another chatbot that suggests food, Muse can operate a bounded set of real café tools. SQLite remains the authority for prices, orders, roles and payments.

The product is **locally hosted operations + cloud Meta inference**, not an offline model or a Meta-endorsed product. The current prototype is for one shop. The name and visual tone evoke a neighborhood Hong Kong café; the workflow applies to other cafés and restaurants.

## Three-minute demo storyboard

The sequence below is a suggested storyboard. For the current recorded MP4, actual viewing guidance and paste-ready submission copy, use the [judge brief](judge-brief.md).

- 0:00–0:20 — Establish the problem and local Mac installation. Show the guest/table entry screen.
- 0:20–1:10 — Guest delegates a meal order. Show the real response, authoritative order details and a follow-up asking what was ordered.
- 1:10–1:40 — Generate/view an actual Muse Image food illustration. Explain that the written order is authoritative and images are optional.
- 1:40–2:15 — Staff moves the order through preparation and service. Show the same order ID.
- 2:15–2:45 — Owner dashboard: actual hourly/daily token statistics, limits, staff/menu management entry points.
- 2:45–3:00 — Close: install on a shop Mac, scan a QR, let Muse handle the conversation while people serve guests.

Use separate disposable data and demo wallets. Never show API keys, personal recovery phrases or real guest data. Disclose any cuts and test-mode fulfillment. Do not label a scripted or offline response as live AI. Preserve raw recordings; provide a caption file alongside the final video.

## Submission checklist

- Participant acceptance and track confirmed in the organizer portal.
- Check the actual submission form for required fields, repository visibility and video limits; the public event page does not specify all deliverables.
- Official page currently lists Nov 4, 2026, 9:00 PM ET = Nov 5, 2026, 11:00 AM KST. Recheck before submission.
- Repository/README accessible to judges; clean setup and `.env.example`; no secret or local DB included.
- Video link accessible without a Tailscale account. The current private demo URL alone is not sufficient for external judges.
- State what is real: Meta guest tools/image generation, local persistence, role checks, usage accounting. State what is a demo: wallet credits and service simulation.
- Do not claim perfect multilingual coverage, full offline AI, automatic accounting or an already deployed production POS.

Event: https://dev.meta.ai/events/global-hackathon

## Suggested closing line

“Put Muse behind the counter. Let people focus on hospitality.”

## Pilot and seed-stage evidence plan

Target first pilot: a single independent café with the owner's Mac and existing phones/tablets. Do not claim paying customers, measured savings, revenue or production deployment without evidence.

Measure against the same shop's manual baseline: time from guest request to accepted order; staff intervention rate; wrong-item/quantity rate; abandonment; Meta requests/tokens per completed order; peak-hour recovery from network failure. Record actual dollars only when the provider billing data is available—tokens alone are not a price.

The next milestone is an owner-supervised service pilot with explicit allergen handling, reliable network access and real payment integration chosen by the merchant. A seed discussion should use that evidence, not a feature-count slide. Current demo credits and synthetic traffic demonstrate software behavior, not business traction.

## Reproduce the screen recording

```sh
# Terminal 1: disposable shop with real, paid Meta guest requests
python3 scripts/e2e_server.py --live-demo
# Terminal 2: waits for actual responses and records only after fixture login
node scripts/record_demo.mjs
```

The video is captured at 1440×900. Generated `.webm`, screenshots and chapter timestamps go to ignored `artifacts/demo/`. The script uses public test identities before recording. Its staff role uses wallet index 1. Recording requires a configured environment/.env Meta key, sufficient budget, and browser dependencies. It never submits the hackathon entry or publishes a video.

Convert a successful recording to MP4 after the recorder finishes:

```sh
python3 scripts/export_demo.py artifacts/demo/NAME_FROM_VIDEO_LOG.webm
```

This produces a 180-second H.264 MP4 (1440×900), an embedded optional English subtitle track, and a separate `.srt`. There is no narration/audio. The raw capture and chapter timestamps are preserved. Check the final MP4 before submitting; neither the recorder nor the exporter hides failed operations.
