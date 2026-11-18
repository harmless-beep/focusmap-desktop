# Hyperframes Composition Brief: FocusMap

## Objective
Create a short launch-style brag video for FocusMap using the `/brag` workflow.

## Output
- Composition directory: `brag-output/composition/`
- Rendered video: `brag-output/brag.mp4`
- Format: landscape — 1920x1080, 30 fps
- Duration: 19 seconds

## Source Material
- Project root: `C:\Users\kroas\Downloads\Programs\rust`
- Primary files read: `README.md`, `docs/marketing.md`, `docs/today.png`,
  `docs/appolis.png`, the locally rendered share-card PNG, `theme.rs`.
- Product name: FocusMap
- Tagline / strongest claim: "Strava for your PC — minus the sharing."
- Key UI moments to use (real shipped captures, no invented data):
  - `docs/today.png` — focus ring, deep minutes, minute-by-minute day map
  - `docs/appolis.png` — pastel city, dotted plaza, ring road, core tower
  - the locally rendered share card (score, goal ring, hour bars, stat tiles)
- Copy that must appear verbatim:
  - "Strava for your PC."
  - "minus the sharing."
  - "Every minute, scored."
  - "Your apps become a city."
  - "A share card, drawn locally."
  - "Tracked locally. Nothing left this device."

## Creative Direction
- Tone preset: `default`
- Creative direction: quietly proud privacy-first launch, with a dry wink
- Interpretation: comfortable 4s beats, clean slides and crossfades, big
  readable type over dark navy. Personality comes from the product, not from
  shouting.
- Angle: see `brag-output/brag-plan.md`.
- Hook: "Strava for your PC." / "minus the sharing."
- Outro / punchline: FocusMap — "Tracked locally. Nothing left this device."
- Avoid:
  - Generic SaaS language
  - Abstract filler visuals
  - Invented screenshots or data; use the real captures above

## Visual Identity
- Background: `#0B0E1A`, card surface `#101528`
- Text: `#F4F6FC`, muted `#9AA4BA`
- Accent: `#8B5CF6`, secondary cyan `#22D3EE`
- Display font: Inter 900
- Body font: Inter 400/700
- Visual references: the app's dark glass cards, violet focus ring, and the
  bright pastel Appolis city.

## Storyboard
Use `brag-output/brag-plan.md` as the creative contract.

Scene summary:
1. Hook — 3s — "Strava for your PC." then "minus the sharing."
2. Reveal — 4s — real Today screenshot + "Every minute, scored."
3. Appolis — 4s — real city screenshot + "Your apps become a city." + slider sweep
4. Share card — 4s — real rendered card + "A share card, drawn locally."
5. Outro — 4s — logo, privacy line, free/MIT/Windows footer

## Audio
- Audio role: warm, upbeat bed with restrained motion-matched accents
- Audio arc: bed in on the hook, soft UI cues on each reveal, one small bell on
  the logo, fade out under the final line
- Music: `assets/music/happy-beats-business-moves-vol-10-by-ende-dot-app.mp3`
  (copy from the brag skill bundle)
- Music treatment: volume ~0.35, fade in over the first 0.4s, fade out from 16s
- Music cue guidance: prefer the bundled preset under
  `assets/music/cues/`; otherwise use `npx hyperframes beats`. Lock at most one
  major reveal to a strong cue within ±0.15s.
- Audio-reactive treatment: subtle; RMS drives a gentle glow on card edges and
  the plaza halo only. No waveform/equalizer visuals.
- Audio-coupled moments:
  - Scene 2 — Today panel landing — soft drop/card cue
  - Scene 3 — Appolis wipe — one short announcement cue; slider ticks
  - Scene 4 — share card landing — soft card cue
  - Scene 5 — logo — single bell, then tail
- SFX selection guidance: sparse, polished; low/medium high-frequency risk
  files. One cue per arrival, never on every word.
- SFX analysis guidance: use the brag skill's `assets/sfx/sfx-analysis.md`
  if available.
- Exact SFX choice: Hyperframes chooses filenames, timestamps and volume after
  the animation is in place.
- Audio files: copy the chosen music and SFX into
  `<output-dir>/composition/assets/`.

## Hyperframes Instructions
Load the Hyperframes skills — `hyperframes` (contract, format, motion, look),
`hyperframes-cli`, `hyperframes-registry`, and `media-use`. This is `/brag`'s
own workflow: skip the generic Hyperframes storyline step, don't read
`references/types/*` or `references/story.md`, don't propose storylines, and
don't write a `SPEC.md`. The brag plan and this brief replace them, including
the LOOK line `references/look.md` asks for. Skip `usage`, `history`, `feedback`
and REVIEW.md; `check` is the gate.

Requirements:
- Show the real FocusMap UI screenshots and the real rendered share card.
- Keep every line of text readable in the final render.
- Total duration 19 seconds.
- Include music and the planned SFX; no voiceover.
- Keep creation and rendering local — no cloud or publishing.
- Run `hyperframes check` before render.
- Documented deviations: natural timing was chosen for all major reveals rather
  than beat locking, so every line stays readable; audio-reactive extraction was
  skipped to keep the shape of the product screenshots exact. Music and SFX are
  still present.

## LOOK
Dark navy `#0B0E1A` ground, violet `#8B5CF6` and cyan `#22D3EE` as the only
saturated accents, Inter 900 for hero lines and Inter 400/700 for captions.
Cards are dark glass (`#101528`) with 1px violet-tinted borders, generous
padding and soft local glows — the app's own visual language. Screenshots sit
on that ground as the hero media, not inside decorative frames. BANNED:
generic gradient washes, stock-photo backgrounds, emoji icons, rainbow
accents, and default "AI video" neon-purple haze.
