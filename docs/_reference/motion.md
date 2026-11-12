---
title: Motion
description: How ZapFastie animates, and the ideas worth building next.
nav_order: 7
---

## What moves today

Motion is one setting, Settings > Appearance > Motion, with three values:

| Value | A transition takes | What it means |
| --- | --- | --- |
| Full | 0.22 s | the default |
| Reduced | 0.1 s | the same transitions, shorter |
| Off | none | every screen is still |

Three things use it. A message that just arrived, sent here or from the phone,
draws a ring around its bubble that widens and fades: one stroke, no re-layout.
A chat that opens fades its messages in over the chat's own background, so
switching conversations reads as a change rather than a cut. A message sent
from here pops the composer's field once, the same way.

Everything animates with `egui`'s own clocks, which stop when the value settles,
so nothing spins. Screens never move for decoration alone: a transition has to
say what changed.

## Rules for anything added

- One stroke, one opacity change, or one transform. Never a re-layout per frame.
- It must stop. If it needs a repaint forever, it is not a transition.
- It obeys Motion, including Off, and it never delays input: typing during a
  transition goes to the field it was typed into.
- It cannot move a control under the pointer. The composer's send button must
  stay where the thumb left it.
- On a screen reader, keep it short or skip it: `fastframe-notifications` and
  the typing dots already step at ten frames a second for that reason.

## Ideas, roughly in order of value

**Conversation**
1. A sent bubble that scales 0.98 to 1.0 as it lands, instead of only its ring.
2. A reply strip that slides up from the composer rather than appearing.
3. A day separator that fades in as the transcript crosses it, instead of the
   sticky chip appearing at once.
4. A jump to a search hit that eases the scroll and flashes the bubble, which
   the jump highlight already half does.
5. Images that fade from their thumbnail to the full picture when they finish
   downloading.

**Lists and navigation**
6. A chat row that lifts and settles when a new message moves it up.
7. A cross fade between Chats, Calls and Settings, on the central panel only.
8. The rail's lit square sliding between destinations instead of jumping.
9. A search field that expands from the search button on a narrow window.
10. The unread badge counting up, or at least fading between values.

**Composer and media**
11. A recording bar that grows from the microphone button when recording starts.
12. The waveform drawing itself as a voice message plays.
13. The picker opening upward from the emoji button with a short spring,
    instead of appearing at a fixed place.
14. A file drop that highlights the whole window, not just the composer.

**Calls**
15. The call screen entering from the bottom on an incoming call, and leaving
    the same way when it ends.
16. A connecting indicator that breathes rather than spins.

**Polish**
17. A theme change that keeps its reveal, which `theme::Transition` already does
    for palettes, and apply the same reveal to a chat's own palette.
18. A duration slider under Motion, 0.1 s to 0.4 s, once there are transitions
    worth tuning.
19. Reduce motion automatically when the desktop says so (`prefers-reduced-motion`
    through the portal), rather than only by hand.

## What not to do

- No bouncing, no overshoot on text, no parallax behind messages.
- No animation on scrolling, selection, or anything that repeats while you read.
- No sound with motion; the notification sounds are the app's only audio voice.
