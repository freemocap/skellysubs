# Interaction layout audit — 2026-10-11

Status: two reproduced layout defects fixed and verified. This is a bounded
conversation/recorder audit, not a guarantee that every application screen is
free of layout shifts. Existing performance changes in the checkout are separate.

## Reproduced defects and implementation

| Interaction | Before | After |
| --- | --- | --- |
| Type, at 1180 and 430 px window widths | Recorder grew 32 px; conversation viewport shrank and scrolled 32 px | No change in recorder/composer/stream geometry |
| Words on an earlier message | Clicked button moved about 26–30 px | Less than 0.5 px movement |
| Pronunciation on an earlier message | Desktop button moved about 14 px | Less than 0.5 px movement |
| Long typed draft | Textarea extended about 41 px below the recorder face | Textarea stays inside the face and scrolls internally |

The voice grid now retains its existing idle height while typing. The draft row
can shrink to that available height, and the textarea no longer writes a competing
content-dependent inline height. Typing focus uses `preventScroll`.

MessageTools now shares the audio inspector's existing scroll compensation with
inline reading toggles. It preserves the clicked control's position while the
message grows, including delayed growth, and stops compensating when the learner
uses the pointer, wheel, touch or keyboard in the scroller. Overflow-menu controls
are outside this fix. Opening a dialog does not engage this anchoring.

## Browser evidence

Used isolated Windows WebView2 desktop instances with a copied local conversation
fixture, no provider responses, and muted playback. Measured element rectangles
and scroll offsets directly, alongside layout-shift observations. A normal CLS
score alone would miss shifts excluded because they followed user input.

The controlled sweep activated DOM buttons at 1180×800 and 430×800. It covered
Words, Pronunciation, translation, analysis, Type, recording settings, microphone
refresh, hold mode, conversation settings and conversation list. Desktop settings
and coach controls were also exercised where present. Narrow-window desktop
emulation is not a physical mobile/coarse-pointer or software-keyboard test.

Other tested controls did not reproduce a comparable surrounding-layout defect.
Explicit coach expansion intentionally changes the coach viewport. Dialog content
can legitimately grow when errors appear; those errors were not hidden to make
geometry appear stable.

Local evidence (ignored, not committed):

- `.local/layout-before.json`, `.local/layout-after.json`: control sweep.
- `.local/layout-details-before.json`, `.local/layout-details-after.json`:
  earlier-message toggles and long-draft geometry.
- `.local/layout-assertions.ts`: saved measurements fail the baseline and pass
  after the changes, with a one-pixel geometry tolerance.
- `.local/workflow/01920d7f-ea07-4db6-950a-ac1516ab816e`: successful replay of the
  existing seven-action recorded session, including its rendered checkpoints.

## Automated verification

- 725 tests passed across 89 reading, media and conversation suites.
- New MessageTools regression verifies toggle anchoring, delayed content growth
  and release after intentional scrolling.
- Existing recording tests cover start/record/transcribe send guards, draft/stream
  switching, stop/discard controls and microphone-check behavior with mocks.
- Application build and `npm run check:fast` passed.

## Remaining coverage limits

The isolated microphone-check attempt returned to idle rather than demonstrating
a successful capture. A separate browser component probe for all recorder phases
did not mount; it supplies no passing geometry evidence and its temporary build
files were removed. Successful live capture, transcription arrival, physical
mobile keyboard behavior, and every settings/progress screen remain outside this
verification. Recording unit tests are not evidence of live device success.

There is no remaining reproduced defect in the measured fixed interactions.
Further changes should start with another concrete reproduction, rather than
freezing all intentional expansion or adding global scroll suppression.
