---
title: WhatsApp on Linux
description: WhatsApp has no Linux app. ZapFastie is a native WhatsApp client for Linux, macOS and Windows, written in Rust, with no browser engine inside.
permalink: /whatsapp-on-linux/
nav_order: 4
seo:
  title: "WhatsApp for Linux: a Native App"
lang: en-US
alternates:
  - lang: en
    url: /whatsapp-on-linux/
  - lang: pt-BR
    url: /pt-br/
---

*This page is also in [português do Brasil](/pt-br/).*

WhatsApp makes apps for phones, Windows and macOS, and none for Linux. On a
Linux desktop you have three choices:

- **WhatsApp Web in a browser tab.** It works everywhere, but it is a tab: it
  closes with the browser, and the page with its browser processes takes about
  a gigabyte of memory.
- **An app that wraps WhatsApp Web.** It gives WhatsApp a window and a tray
  icon, but inside it is still the web page and a browser engine.
- **A native client such as ZapFastie.** It speaks WhatsApp's linked-device
  protocol itself, with no web page and no browser engine.

ZapFastie is a single binary written in Rust. In our Linux test it opens in
under a second and uses about 200 MB of memory, against 1.13 GB for WhatsApp
Web and its Chromium processes. [See the measurements](/benchmarks/).

<img class="VPImage dark" src="{{ '/screenshot.png' | relative_url }}" alt="ZapFastie showing a conversation with an attachment, voice messages, reactions, a quoted reply, and a link preview" width="1280" height="960">
<img class="VPImage light" src="{{ '/screenshot-light.png' | relative_url }}" alt="ZapFastie showing a conversation with an attachment, voice messages, reactions, a quoted reply, and a link preview" width="1280" height="960">

## Install it

ZapFastie has no published package yet: the project has not published a
release, so there is no `.deb`, `.rpm`, Flatpak, AppImage or installer to
download. What
works today is building it, which takes a few minutes once the build
dependencies are in place.

| Distribution | How |
| --- | --- |
| Any Linux distribution | Clone the repository and run `cargo install --path .`, then start `zapfastie`. [Getting started](/getting-started/) lists the packages the build needs. |
| Arch, Manjaro, CachyOS, Omarchy | The same build. `packaging/arch/zapfastie/` holds the packaging template the release workflow turns into an AUR package, and that package is not published yet. |
| NixOS and Nix | `nix run .#zapfastie` from a clone, or install the flake's package. The flake has a Home Manager module. |
| A launcher entry | After `cargo build --release --locked`, run `packaging/install-user.sh` to install the binary, the desktop file and the icon under `~/.local`. |

Then open ZapFastie and link it from your phone, under **Linked devices**,
with the QR code or an eight-character code entered on your phone.
[Getting started](/getting-started/) walks through it.

## What it does on Linux

- **Your chats, stored on your computer.** Messages are kept in an encrypted
  file, with the key in your desktop's keyring, and older messages come from
  your phone as you scroll.
- **Your desktop's look.** Light, dark or the system's, your own colour
  themes, and on Omarchy your desktop theme.
- **It stays running.** Closing the window keeps ZapFastie in the tray, and
  notifications open the chat at the message they announced.
- **Several numbers.** Link more than one WhatsApp number and switch between
  them from your picture at the foot of the rail beside the chat list.
- **Voice and video calls.** Place and answer one-to-one calls, with your
  choice of microphone, speaker and camera.

[Everything ZapFastie does](/what-is-zapfastie/), and what it does not do yet.

## Questions

### Can you install WhatsApp on Linux?

Not WhatsApp's own app: there isn't one. You can use WhatsApp Web in a
browser, or install a client such as ZapFastie, which links to your account
the way WhatsApp Web does.

### How do I use WhatsApp on Ubuntu?

Build ZapFastie from source, as [Getting started](/getting-started/) describes,
and run it. Then scan the QR code with your phone under **Linked devices**.

### Does my phone need to stay on?

No. ZapFastie is a linked device, like WhatsApp Web, so it sends and receives
messages while your phone is off. Older history comes from your phone, so
scrolling further back than ZapFastie has stored waits for the phone to be
online.

### Is it free?

Yes. ZapFastie is open source under the MIT licence, and the code is
[on GitHub](https://github.com/joao-pedro-gms/zapfastie).

### Is it safe for my account?

ZapFastie is an unofficial client, and using one may be against WhatsApp's
terms. It links as a normal companion device and never automates or sends
messages in bulk, but WhatsApp may still restrict accounts that use
unofficial clients. [Account safety](/what-is-zapfastie/#account-safety) says
more.
