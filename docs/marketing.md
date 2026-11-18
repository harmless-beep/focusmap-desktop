# Marketing kit — FocusMap

Everything here is reusable copy for the launch. The product facts come straight
from the README and the shipped v1.0.0 build: local-only, no account, no cloud,
no telemetry, Windows 10/11.

**Links**

- Repo: https://github.com/harmless-beep/focusmap-desktop
- Download / installer: https://github.com/harmless-beep/focusmap-desktop/releases/latest
- Screenshots: `docs/today.png`, `docs/appolis.png`

---

## Taglines

Pick one and use it everywhere.

1. **Strava for your PC — minus the sharing.**
2. **See where your focus went. Keep it all on your machine.**
3. **Your workday, drawn as a little city.**
4. **A private focus map for people who actually use a computer.**
5. **No account. No cloud. Just an honest map of your day.**

## One-liner

FocusMap is a free Windows app that turns your keyboard, mouse and app activity
into a minute-by-minute focus score — and a small city that grows out of the apps
you used. Nothing is uploaded, ever.

## 30-second pitch

Most productivity tools ask you to log in and send your activity to somebody's
server. FocusMap does the opposite. It sits in your tray, counts input *timing*
(not what you type), and scores each minute of your day. At the end of the day
you get a clear map of when you were really in flow — and Appolis, a tiny city
where every app is a building and every tall tower is time you spent there.
There's no account, no cloud, and no telemetry. It's MIT-licensed and built in
Rust.

## Benefit-led highlights

Lead with these, not the feature names.

- **Find your real deep-work hours.** A minute-by-minute map shows when you were
  actually in flow, not just when your computer was on.
- **Understand the cost of switching.** Every window change lowers the score, so
  the scattered hours are impossible to miss.
- **Watch the day rebuild itself.** The replay slider turns Appolis back into
  any moment of the day — a surprisingly quick way to *feel* where the time went.
- **A share card you control.** Two 1080 × 1080 PNGs drawn locally, saved only
  when you ask. No watermark, no upload.
- **Quiet by design.** Roughly 0.1% CPU on the timeline, about 1–2% with the
  city open. Low preset available.
- **Private by architecture.** Only counts, app executable names and your own
  tags are stored. No keystrokes, no titles, no URLs, no network calls.

## Proof points (safe to quote)

- Keyboard hook compares only the virtual-key code to `VK_BACK` to count
  backspaces; it never reads characters or shortcuts.
- Foreground tracking stores the executable name (e.g. `Code.exe`), never the
  window title or URL.
- No server, no analytics, no crash upload, no invisible web view.
- Exports: CSV and JSON from the Privacy screen; one-click wipe.
- MIT licensed; three Rust crates, tests passing locally via
  `cargo test --workspace`.
- Free download: portable EXE or MSI installer.

---

## Launch posts

### X / Twitter (5-post thread)

1. I built the app I wanted for my own workdays: a private focus map that never
   leaves my PC. It's called FocusMap, and it's free + open source. 🧵
2. It counts input timing, not what you type. No keystrokes, no window titles,
   no URLs, no cloud. Just counts, app names, and the tags you add yourself.
3. Every minute becomes a score. Deep work goes green, scattered time goes
   purple. The day map makes it very obvious which hours you keep blowing on
   tab-hopping.
4. Then there's Appolis: a little toy city where each app is a building. Taller
   tower = more time. Replay the day with a slider. Daylight and Blueprint modes.
   Yes, it's ridiculous. Yes, it's useful.
5. Windows 10/11. MSI installer or portable EXE. MIT licensed.
   https://github.com/harmless-beep/focusmap-desktop
   Feedback welcome — especially the thing you'd want it to do next.

### Reddit — r/productivity or r/getdisciplined

**Title:** I made a local-only focus map that turns your workday into a tiny city

**Body:**

I wanted to know where my focus actually went without installing something that
uploads my activity. So I built FocusMap.

It counts keystroke *timing* and window switches, never the content, and scores
each minute from 0–100. The Today screen shows the whole day at a glance. The
fun part is Appolis: every app becomes a building in one of five districts, and
a slider rebuilds the city for any moment of the day.

Details that matter to me:

- no account, no cloud, no telemetry; everything is in local SQLite
- no keystroke content, no window titles, no URLs, no screenshots
- CSV/JSON export and a one-click wipe
- MIT licensed, Windows 10/11, installer or portable EXE

Repo and downloads: https://github.com/harmless-beep/focusmap-desktop

Happy to answer questions about the privacy design or the scoring model.

### Hacker News

**Title:** Show HN: FocusMap – a local-only focus tracker that draws your apps as a city

**Comment:**

I built FocusMap because every focus tool I tried wanted an account and an
upload. This one runs entirely on the machine: a Windows hook counts input
events (only the virtual-key code for backspace is inspected, no characters),
and the foreground poller stores the executable name, never the title or URL.

Each minute is scored from continuity, steadiness, switching and a few category
caps. The UI is egui; the city (Appolis) is wgpu inside an egui paint callback —
one instanced pass, 4x MSAA, capped frame rate. The share cards are drawn
locally with tiny-skia.

Rust, MIT, no services. Downloads and source:
https://github.com/harmless-beep/focusmap-desktop

### Product Hunt

**Tagline (60 chars):** A private focus map that turns your apps into a city

**Description:**

FocusMap is a free, local-only focus tracker for Windows. It scores each minute
of your workday from input timing and window switches — never what you type —
and shows it as a minute-by-minute map. Appolis turns the same data into a small
city where every building is an app you used, with a replay slider, daylight and
blueprint themes, and share cards rendered on your machine. No account, no
cloud, no telemetry. MIT licensed.

**First comment:**

Hey Product Hunt! I'm the builder. The short version: I wanted to know when I
was actually in flow without sending my activity anywhere, so I made FocusMap.
It only stores counts, executable names and your own tags — no keystrokes, no
titles, no URLs, no network calls. Everything is in local SQLite and you can
wipe it in one click. I'd love to hear what you'd want next.

---

## Handling common objections

- **"Is this spyware?"** It stores counts and executable names only. The keyboard
  hook checks one key code (`VK_BACK`) to count backspaces; it never reads input
  content. There is no network code path in tracking at all.
- **"Why a city?"** A minute map is accurate but abstract. Seeing "that huge
  tower is Slack" lands differently, and the replay slider makes the shape of a
  day obvious in seconds.
- **"Windows only?"** Yes, for now. The capture layer is Windows-specific; the
  scoring, storage and stats core is platform-independent.
- **"Is it heavy?"** Timeline is about 0.1% CPU. Appolis is roughly 1–2% at the
  balanced preset; there's a Low preset that drops ambient animation.
- **"Why is memory high?"** The wgpu/Vulkan stack costs roughly 240–315 MB
  working set. Honest trade for the city; the timeline alone is much lighter.

## Call to action

Use the same line everywhere:

> Free, private, and open source. Get it on Windows:
> https://github.com/harmless-beep/focusmap-desktop/releases/latest
