# Native menus and command palettes

The Blitz shell connects the shared Menu, Menubar, ContextMenu and Palette
protocols. Sapid owns menu membership, matching, checked/radio values, submenus,
controlled open state and delays. These are self-drawn semantic controls; platform
system menus, application menu bars and tray services remain separate work.

## Input and placement

Keyboard input uses the shared `KeyboardInput` commands and current mounted
ancestry, including RTL, Unicode typeahead, trigger first/last, submenu open/close
and Tab exit. The nearest Menu is a boundary even without an interested listener.
Enter activates an intrinsic menu checkbox/radio exactly once; Space retains its
ordinary activation. Disabled, inert, hidden, composing and retired controls do
not supply input. Palette shortcuts use the nearest explicit Palette/Keyboard
scope, preserving editor input and the shared modifier/repeat restrictions.

Context requests come from mouse/pen context events or the shared keyboard chord.
The nearest declared region is a boundary, including an unsubscribed region.
`Native` preserves editor context behavior; `Override` explicitly admits a custom
editor menu. Touch long-press policy is not added. Pointer coordinates are finite
viewport CSS pixels. Keyboard coordinates use the origin's actual border box.
ContextPoint Menu layers clamp to the viewport with an eight-pixel margin and
retain the current parent modal and focus-scope restrictions.

Layer placement and outside hit testing compensate only the queried node's own
scroll offset in Blitz's client rect. Ancestor and viewport scrolling stay in the
transform. This fixes false outside dismissal when clicking a visible bottom
item in a scrolled menu. The original failing regression is preserved at
`/Volumes/Data/Caches/glory/menu-scroll-before.log`.

## Focus and explicit reveal

Sapid retains one current NodeId focus request and delivers it after visibility
bindings commit. It requests Focus and, where supported, explicit ScrollIntoView.
Focus itself keeps its prevent-scroll contract. Closing/Tab clears an older
request even when the application refuses the controlled close, so a queued
navigation cannot reclaim focus after exit. No host objects or visited-item
history enter widget state.

ExitMenu resolves the current declared return target after controlled changes.
It accepts its own hidden Menu when close and exit commit together; suspended,
disconnected, inert and disabled ranges remain rejected. Tab uses current mounted
physical order within the active modal/focus scope, with a 65,536-node traversal
budget and current radio tab-stop policy. It excludes all Menu subtrees and exits
the entire Menubar or context region, rather than entering another root trigger
or a context descendant. Reaching an end clears focus without wrapping. Headless
renderers explicitly refuse focus work.

The pinned Blitz fork also cancels a prior touch fling on an explicit programmatic
scroll. Fling frames retain their own user scroll chain. The original failure is
preserved in `menu-fling-before.log`; fork tests cover cancellation and continuing
fling frames, and the adapter's touch/wheel/Home regression verifies the consumer
behavior. See [engine provenance](engine-forks.md) for the fixed source.

## Verification and remaining boundaries

Native regressions cover commands, RTL/typeahead, nearest context/shortcut
boundaries, editor/composition guards, clamped placement, controlled close/refusal,
Tab, suspension, scrolled pointer hits, 1x/2x CPU output and current AccessKit
MenuBar/MenuItem/checkbox/radio/modal relationships. Model/CPU evidence does not
establish OS screen-reader announcements.

The isolated `/Volumes/Data/Caches/glory/native-menu-consumer` mounts actual Sapid
widgets. Its eight tests cover dropdown checks/radios/submenus, Menubar switching,
context placement/Tab, Command/Spotlight Chinese search/commit/modal restore,
initially open submenus, controlled refusal, long-menu reveal/scrolled clicks,
and queued navigation followed by a new Palette modal. Four themes are exercised
where applicable. The existing five choice consumers remain a separate regression.

The mobile `menu-probe` displays a Menubar, managed Menu, disabled and Chinese
items, checkbox, context region and Palette dialog. SDK builds, installation,
launch, installed-binary hashes and initial frames are recorded separately from
real touch/keyboard, VoiceOver/TalkBack and signed-device acceptance. Evidence is
stored under `/Volumes/Data/Caches/glory/native-menus`; final counts and hashes
belong to its `result.json`. Browser checks of the shared Sapid focus change use
the existing Menu/Menubar/ContextMenu/Command acceptance scripts.

Tooltip/navigation Group layers, Sheet gestures, layer Translation, intrinsic
collapse motion, rich editing, file pickers and semantic clipboard remain separate
native gaps. The subsequent [TagsInput protocol](native-tags.md) retains its own
platform clipboard/input acceptance boundaries. Multi-scene/device/distribution and other application
services remain tracked by the [feature-gap report](dioxus-feature-gap-analysis-2026-10-26.md).
