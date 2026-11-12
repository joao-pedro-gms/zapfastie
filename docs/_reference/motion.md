---
title: Motion
description: How ZapFastie animates, what the settings do, and the ideas still open.
nav_order: 7
---

## What moves today

Motion is Settings > Appearance > Motion, with three values:

| Value | A full transition takes | What it means |
| --- | --- | --- |
| Full | the pace below | the default |
| Reduced | half the pace | the same transitions, shorter |
| Off | none | every screen is still |

The pace is a slider under the picker, 0.10 s to 0.40 s, and it sets how long a
full transition lasts. It hides while Motion is Off: there is nothing to pace.

Ten things use it, all of them on `egui`'s own clocks, which stop when the value
settles, so nothing spins:

**Conversation**

- A message that just arrived, sent here or from the phone, draws a ring around
  its bubble that widens and fades: one stroke, no re-layout. A message sent
  from here also fills with its own accent for a moment and fades back, so it
  reads as arriving rather than as already being there.
- Sending a message pops the composer's field once.
- Opening a chat fades its messages in, over the chat's own background.
- The reply strip fades up from the composer instead of snapping into place.
- The sticky date at the top of the transcript fades out a second after the
  reader stops scrolling.

**Every change of view**

One wash of the interface's own background covers the list and the pane, and
fades out in the Motion time. It plays on any change of view, not only on a
page: another page (Chats, Calls, Settings), another chat list filter or label,
the archive, the locked folder, and another conversation. Typing in search does
not play it: a keystroke is not a change of screen.

The key is `view_stamp` in `src/ui/mod.rs`, a hash of exactly those fields, and
`every_change_of_view_moves_the_stamp` fails if one of them stops moving it.
- The emoji, GIF and sticker panel rises the last ten points into place.
- Dragging files over the window washes the whole window in the accent colour
  and draws a border on it, not only the card in the middle.

**Where the code lives**

`App::motion_time()` is the only answer to "how long is a transition right now":
the Motion choice at the configured pace. Every animation below reads it, and a
zero puts the end state on screen at once.

## Rules for anything added

- One stroke, one opacity change, or one transform. Never a re-layout per frame.
- It must stop. If it needs a repaint forever, it is not a transition.
- It obeys Motion, including Off, and it never delays input: typing during a
  transition goes to the field it was typed into.
- It cannot move a control under the pointer. The composer's send button must
  stay where the thumb left it.
- On a screen reader, keep it short or skip it: `fastframe-notifications` and
  the typing dots already step at ten frames a second for that reason.

## Still open

**Conversation**

1. A jump to a search hit that eases the scroll and flashes the bubble, which
   the jump highlight already half does.
2. Images that fade from their thumbnail to the full picture when they finish
   downloading.
3. The waveform drawing itself as a voice message plays.

**Lists and navigation**

4. A chat row that lifts and settles when a new message moves it up. A row's
   height is its layout, so this needs the list to paint at interpolated
   positions instead of laying out each frame.
5. The rail's lit square sliding between destinations rather than jumping.
6. A search field that expands from the search button on a narrow window.
7. The unread badge counting up, or at least fading between values.

**Composer and media**

8. A recording bar that grows from the microphone button when recording starts.

**Calls**

9. The call screen entering from the bottom on an incoming call, and leaving the
   same way when it ends.
10. A connecting indicator that breathes rather than spins.

**Polish**

11. Applying the palette reveal `theme::Transition` already does for a theme to
    a chat's own palette, when the chat changes its colours.
12. Reduce motion automatically when the desktop says so
    (`prefers-reduced-motion` through the portal), rather than only by hand.

## What not to do

- No bouncing, no overshoot on text, no parallax behind messages.
- No animation on scrolling, selection, or anything that repeats while you read.
- No sound with motion; the notification sounds are the app's only audio voice.
