# Coaxi Built-in Teammate Asset Notice

The four current original anime teammate portraits were created on 2026-10-24
using native OpenAI image generation at the Coaxi project owner鈥檚 request.
The owner requested replacement of the previous companion artwork and its use
in the public repository and packaged application. These are original characters,
not third-party character artwork or rights to third-party trademarks.

- `source.avif`: packaged 768 脳 960 portrait, AVIF quality 80.
- `icon-192.png`: 192 脳 192 face rendition derived from the same original portrait.
- `crates/rovai-core/resources/member-avatars/*.png`: 768 × 960 PNG portraits from the same artwork for Host/Web/mobile delivery.
- Existing `rovai://member-avatar/builtin/{role}/v1` references and role IDs remain
  stable for installed data. User-managed avatars are not replaced.
- Preset names and model-visible identities follow the separately approved
  `coaxi-builtin-member-names` revision 1. Artwork adds no model persona or permissions.
- Previous artwork remains available in Git history. Full generated masters and
  before/after evidence are retained outside the source tree in the task assets.

## Curated file digests

| File | SHA-256 |
|---|---|
| `apps/desktop/src/renderer/src/assets/characters/luoke/source.avif` | `c4131d36f7a07b6f3871ed42f2cb13b1786ab72453eb146de1bd5caad66c5d32` |
| `apps/desktop/src/renderer/src/assets/characters/luoke/icon-192.png` | `e9c29a9b75861b1acdd70e24b59d191298692c74cef9c8444c04f465f0a4d2bd` |
| `crates/rovai-core/resources/member-avatars/luoke.png` | `5f0a2a0ab020b995bf9d4ce7ee08b94ae49858e3a57416d54a358ccef295ad46` |
| `apps/desktop/src/renderer/src/assets/characters/muwa/source.avif` | `042b6e8722c1751b378937c0df661b2e92be7f5816f24172f570047314180bc1` |
| `apps/desktop/src/renderer/src/assets/characters/muwa/icon-192.png` | `46f5c687aa102f7b1e648414fb6a8b9f304fe5cd9207c4d9f87dbb72301f087d` |
| `crates/rovai-core/resources/member-avatars/muwa.png` | `70125fdd977afa9df4f36d43b856c6187de46391cf9b304395936fc5236a36cd` |
| `apps/desktop/src/renderer/src/assets/characters/mianzhi/source.avif` | `fba6f30d78f4902d6063799d3e8644378b29803500b834b4669dea155310c2fb` |
| `apps/desktop/src/renderer/src/assets/characters/mianzhi/icon-192.png` | `df8183b6ec8452fd05f15d02ba1f905a08c6b3e5acf471e001c66336482b2707` |
| `crates/rovai-core/resources/member-avatars/mianzhi.png` | `021263b0858a69e1b2fee6199ce26cabcda728e17f197f7a8130207c3a0270e9` |
| `apps/desktop/src/renderer/src/assets/characters/qilu/source.avif` | `62c8258525291ea083a9ad3fe2c9ed8e8a9dca037beadc162a64cfb4ba5ecf56` |
| `apps/desktop/src/renderer/src/assets/characters/qilu/icon-192.png` | `34210844be81735fefa15f9bb2fc54dc3135a8503f03979938705256e8dc457e` |
| `crates/rovai-core/resources/member-avatars/qilu.png` | `bd42c816a2186641a2eaa1dbecf28af1890c8ef67af05f65a2fb3f80e0ac4f9a` |
