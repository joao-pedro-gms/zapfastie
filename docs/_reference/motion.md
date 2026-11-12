---
title: Motion
description: Why ZapFastie is still, and the transitions it could have one day.
nav_order: 7
---

## The interface is still

Screens change at once. There is no fade, no slide and no pop, and no Motion
setting to turn any of them on: an animated interface was tried and read as
noise over the messages, which are the thing being read.

What is still there, because it is not decoration:

- the sticky date at the top of a transcript fades out a second after the
  reader stops scrolling, instead of vanishing on the frame they stop;
- the typing dots and the call indicator step about ten frames a second, which
  is slow enough not to keep the window's thread busy while a screen reader is
  attached (`src/motion.rs`, `src/animation.rs`);
- animated stickers, GIFs and motion photos play, because they are the content.

## If it comes back

Anything added would have to answer to these, which is where the first attempt
went wrong: it had a transition for every change of view, and a transition for
every change of view is the same as no hierarchy at all.

- One stroke, one opacity change, or one transform. Never a re-layout per frame.
- It must stop. If it needs a repaint forever, it is not a transition.
- It never delays input, and it never moves a control under the pointer.
- One transition per change of *state*, not per repaint: typing in search, a new
  message arriving, and a scroll are not screen changes.
- Settings > Appearance would need a switch, and Off would have to be the
  default for anyone who turns it on once and dislikes it.

## Ideas, in the order they would earn their place

1. A theme change that cross-fades the palette, which `theme::Transition`
   already does, rather than every screen change doing it.
2. A jump to a search hit that eases the scroll and flashes the bubble it
   landed on.
3. Images that fade from their thumbnail to the full picture when a download
   finishes.
4. The call screen entering from the bottom on an incoming call, and leaving the
   same way when it ends.

Everything else — bubbles popping, panels rising, panes washing, chat rows
lifting — was built once and removed. It is worth re-reading the rules above
before building it again.

## What not to do

- No bouncing, no overshoot on text, no parallax behind messages.
- No animation on scrolling, selection, or anything that repeats while you read.
- No sound with motion; the notification sounds are the app's only audio voice.
