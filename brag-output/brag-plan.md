# Brag Plan: FocusMap

## What is this app?

A private Windows focus tracker that scores each minute of your workday and
draws it as a map — and as Appolis, a little city where every app you used is a
building. No account, no cloud, no telemetry.

## The angle

Privacy apps usually lead with a lock icon and a legal disclaimer. This one
leads with the fun part: your day is a tiny toy city. The joke is the contrast —
"Strava for your PC" sounds like the most shared thing imaginable, and the very
next line is "minus the sharing." The video shows the actual app doing its thing
(screenshots from the shipped build), then lands on the quiet promise that
nothing left the machine.

## Hook (first 2-3 seconds)

Full-frame dark navy. One line slams in, big and confident:
**"Strava for your PC."**
A beat later, smaller and dry: **"minus the sharing."**
No logo yet. The contrast is the hook.

## Key moments (the middle)

- The real Today screen slides in: focus score ring, deep minutes, the 24-hour
  minute map. Line: **"Every minute, scored."**
- The real Appolis daylight screenshot wipes across: pastel districts, dotted
  plaza, tower under the core. Line: **"Your apps become a city."** A replay
  slider sweeps across the bottom edge to suggest scrubbing the day.
- The real 1080 share card appears as a card: gradient ring, hour bars, stat
  tiles. Line: **"A share card, drawn locally."**

## Outro / punchline

Back to dark. Logo dot + **FocusMap**. Under it:
**"Tracked locally. Nothing left this device."**
Small footer: **"Free · MIT · Windows 10/11"**

## User flow worth showing

Entry → key action → result:

1. Open Today and see the minute map fill in as the day is scored.
2. Switch to Appolis and hover/scrub a tower to rebuild a moment.
3. Open Share card and save the locally rendered PNG.

The centerpiece scenes use the real shipped UI captures from `docs/today.png`,
`docs/appolis.png` and the locally rendered share-card PNG. No invented data is
shown: the captures come from the app's own demo profile.

## Tone

- Preset: `default`
- Creative direction: quietly proud privacy-first launch, with a dry wink
- Interpretation: comfortable 4-second beats, clean crossfades and slides,
  big readable type over dark navy. The product's own oddness (a city made of
  app usage) supplies the personality; no shouting, no SaaS clichés.

## Format: landscape — 1920x1080
## Duration: 19 seconds

## Visual identity (from the project)

- Background: `#0B0E1A` (app canvas), card surface `#101528`
- Accent: `#8B5CF6` (violet) with cyan `#22D3EE` as secondary
- Text: `#F4F6FC`, muted `#9AA4BA`
- Display font: Inter 900
- Body font: Inter 400/700
- Strongest visual element: the Appolis daylight diorama — pastel towers around
  a glowing dotted plaza — with the purple focus ring as a secondary motif.

## Share copy (draft)

Strava for your PC, minus the sharing. FocusMap scores your focus minute by
minute and turns your apps into a little city. Nothing leaves your machine.

## Audio direction

- Role: warm, upbeat bed with restrained motion-matched accents
- Music: `happy-beats-business-moves-vol-10-by-ende-dot-app.mp3` (compact, punchy)
- Music treatment: start at a low bed, fade out under the final line
- Music cue guidance: use the bundled cue preset if present; otherwise detect at
  composition time. One strong cue for the Appolis reveal, one for the outro.
- Audio-reactive treatment: subtle; card glow and the plaza halo breathe gently
  with RMS. No waveform or equalizer visuals.
- SFX posture: sparse-to-moderate; one soft UI cue per reveal, a short
  announcement at the logo, no aggressive hits.
- Audio-coupled moments: Today panel slide-in, Appolis wipe, share-card landing,
  final logo.
- Restraint rule: audio never competes with the on-screen text; no strobing or
  heavy pulsing.

## Storyboard

### Scene 1 — Hook — 3s
Dark navy field. "Strava for your PC." lands large; "minus the sharing." appears
below a beat later, tinted cyan. No product UI yet.
Sequential/interaction: yes — two lines arrive one after the other, the second
holding at least 1.2s.
Audio intent: confident, slightly playful; a single soft UI tick.
Audio-coupled idea: second line lands on a soft drop cue.
Music: upbeat bed begins.
Transition mood: clean slide → Scene 2

### Scene 2 — Reveal — 4s
The real `docs/today.png` screenshot slides in from the right at a slight scale,
holding as the hero. Caption at left: "Every minute, scored." Small violet focus
ring accent in the corner.
Sequential/interaction: yes — panel arrives, then caption.
Audio intent: warm confirmation.
Audio-coupled idea: panel landing gets a soft card/drop sound.
Music: bed continues.
Transition mood: clean wipe → Scene 3

### Scene 3 — Appolis — 4s
The real `docs/appolis.png` screenshot fills the frame with a horizontal wipe.
Caption: "Your apps become a city." A thin replay-slider line sweeps along the
bottom edge to suggest scrubbing the day.
Sequential/interaction: yes — image wipe, then slider sweep, then caption.
Audio intent: the lift of the reveal.
Audio-coupled idea: one soft announcement cue at the wipe; tick marks on the
slider sweep.
Music: strong-cue lock here if a preset is available.
Transition mood: soft crossfade → Scene 4

### Scene 4 — Share card — 4s
The real rendered share card (locally drawn 1080 PNG) comes forward as a card,
slightly tilted, with the caption "A share card, drawn locally."
Sequential/interaction: yes — card scales in, caption follows.
Audio intent: tidy, satisfying.
Audio-coupled idea: card landing sound.
Music: bed continues toward its fade.
Transition mood: crossfade → Scene 5

### Scene 5 — Outro — 4s
Clean dark field. Violet brand dot, "FocusMap", the line "Tracked locally.
Nothing left this device.", and a small "Free · MIT · Windows 10/11".
Sequential/interaction: yes — dot and name, then line, then footer.
Audio intent: quiet landing; music fades under the final line.
Audio-coupled idea: single short bell on the logo, then tail.
Music: fade out.
Transition mood: hold to end.

**Music mood for this video:** upbeat but restrained.
**Audio summary:** a warm, punchy bed carries the four reveals; three soft UI
cues mark the arrivals and one small bell marks the logo, then the music fades.
