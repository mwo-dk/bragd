[← README](../../README.md) · [Docs index](../README.md)

# Getting started

Installing Bragd, the first start, the install wizard and What's new.

Each feature in this area gets a page of its own, listed here by the pull request that brings the feature.

## The install wizard

On the first start Bragd opens the install wizard: six short steps. Every step can be skipped, and each one names the Settings area that holds the same choices. Closing the wizard in any way (Done, ×, Esc) counts as having seen it. *Settings → Overview → Run the install wizard again* opens it once more.

| Step | What you choose | Settings area |
|---|---|---|
| 1 Welcome | Language (each by its own name), Theme and Board theme, with a preview | Look |
| 2 Game families | Which Game families and Variants are on | Game families |
| 3 Engines | Nothing: it shows the Engines found on this machine for each Family that is on | Engines |
| 4 Knowledge | Downloads: TWIC every week and Syzygy 3–5 pieces (about 1 GB) are ticked; the others are listed with their sizes | Knowledge |
| 5 Online | Nothing yet: signing in to servers comes in a later version | Online |
| 6 Helper | *Start with my session* | Knowledge |

Choices in steps 1, 2 and 6 are saved at once, as in Settings. Nothing is downloaded until you press **Done** on the last step, which lists what will happen. Then Bragd turns the chosen Sources on in the Helper and starts the Syzygy download. **Skip** on step 4 unticks every download. The wizard adds up the sizes and warns when the downloads would leave less than 5 GB free on the disk that holds Bragd's data.

![The install wizard's Welcome step](../screenshots/wizard-welcome-1100.png)

![The Knowledge step, with sizes and the total](../screenshots/wizard-knowledge-1100.png)

## What's new

After an upgrade, the tour cards of every version newer than the one you last saw open in the same frame as the wizard, one card at a time. Closing them marks them seen. *What's new* in the sidebar lists every version with its cards.

![A What's new card](../screenshots/whats-new-card-1100.png)

## Help (F1)

F1, or *Help* in the sidebar, lists every feature: what it does and the questions people ask, with *Show me* opening its page here. The filter finds every word you type, whatever its case or accents: `grunfeld` finds *Grünfeld*.

Notices and hints show in a small area in the bottom corner. A notice (a mistake in `config.toml`, a newer version) shows until you close it. A hint shows on the first three starts. The update notice names the newest version and the command that upgrades your copy, with a *Copy* button. The update check is turned off in *Settings → Privacy and updates*.

![Help (F1)](../screenshots/help-1100.png)

## Settings → Overview

| Button | What it does |
|---|---|
| Run the install wizard again | Opens the wizard |
| Open config.toml | Opens `config.toml` in your system's editor, making it empty first when there is none |
| Suggest a better translation | Opens a new GitHub issue in the browser with your language and version. Type the text to mend first, and its key is filled in too. Bragd itself sends nothing |

## Questions

#### Does the wizard download anything before I press Done?

No. Steps 1 to 6 only save settings. The downloads you ticked start when you press Done on the last step.

#### Why is a Source greyed out?

This version lists it so you know it is coming, but cannot fetch it yet.

#### How do I see the What's new cards again?

*What's new* in the sidebar lists every version with its cards.

---
[← Previous: Docs index](../README.md) · [Next: Play →](../play/README.md)
