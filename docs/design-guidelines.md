# Design guidelines

Research-backed rules for the builder (web app) and the printed character
sheet, and the concrete decisions they lead to. Each rule cites its source;
"→" lines are how this project applies it.

## 1. The creation wizard

**Use a wizard for long, sequential, branching work; label steps and show
where the user is.** Wizards suit lengthy processes with dependencies; steps
should carry descriptive labels, and a sequence map lets people revisit
earlier steps without losing work ([NN/G: Wizards][nng-wizards],
[NN/G: complex applications][nng-complex]). Visa's design system recommends
multi-page wizards for flows with branching and grouping by user
expectations ([Visa][visa-wizard]).
→ Nine steps in rules order (class → background → species → abilities →
spells → equipment → details → review), a persistent step map with
per-step completion, "Step n of 9", and free navigation between steps.

**Always offer Back and keep the user's data.** NN/G lists the lack of an
explicit Back as a wizard usability gap and advises never breaking the browser
Back button ([NN/G: user control and freedom][nng-control]).
→ Back/Next on every step; steps are URLs (hash routes), so the browser Back
button works; nothing is lost on navigation; undo/redo for every edit.

**Remind users of earlier answers.** Splitting a process into chunks risks
users forgetting what they chose earlier unless the design reminds them
([Eleken][eleken]); WCAG 2.2 *Redundant Entry* (3.3.7, A) requires not asking
for the same information twice ([W3C: new in 2.2][w3c-22]).
→ A live character summary beside every step (wide screens) and in the
header; the name is one field shown wherever it is needed.

**Summarise what is left, link to each item, place it consistently.**
GOV.UK shows an error summary at the top of the page, even for one error,
linking each item to its field, worded the same as the inline message
([GOV.UK: error summary][govuk-error]). WCAG 2.2 *Consistent Help* (3.2.6, A)
asks for help to stay in the same relative place ([W3C][w3c-22]).
→ Each step opens with the same "Left on this step" box listing unfinished
choices as links that jump to them; the Review step lists everything.
Guidance text sits under each step heading, always in the same place.

**Validate early, specifically, and clear messages once fixed.** Baymard's
2024 testing found live inline validation improves error recovery; messages
should be specific to the rule that failed and disappear once resolved
([Baymard: inline validation][baymard-inline],
[Baymard: adaptive error messages][baymard-adaptive]).
→ Every choice shows a live "n / m" counter; disabled options state *why*
("Requires proficiency", "Already known"); warnings (duplicates) never block.

**Prefer visible options to dropdowns for small sets; never use placeholders
as labels.** Radio buttons beat dropdowns below about five options because
all choices stay visible ([NN/G: radio vs. checkbox][nng-radio],
[Baymard: drop-downs][baymard-dropdown]); placeholders disappear and are
mistaken for data ([NN/G: placeholders][nng-placeholder]); labels go above
fields ([Baymard: form design][baymard-form]).
→ Choices render as visible option lists; dropdowns only for long ordered
sets (levels, standard-array values); every input has a persistent label.

**Show system status.** Progress indicators reduce uncertainty while waiting;
skeletons suit full-page loads under ~10 s ([NN/G: progress
indicators][nng-progress], [NN/G: skeleton screens][nng-skeleton]); empty
states should say what to do next ([NN/G: empty states][nng-empty]).
→ Branded loading screen with progress while the compendium loads, a clear
error with retry, and an actionable empty state on the character list.

## 2. Visual design and accessibility

**Contrast.** Text ≥ 4.5:1 (≥ 3:1 for large text); UI component boundaries
and focus indicators ≥ 3:1 ([web.dev: color and contrast][webdev-contrast]).
→ Tokens are checked by script (`docs/design-guidelines.md#tokens`); colour is
never the only signal (checkmarks, text badges, counters).

**Target size and focus.** Pointer targets ≥ 24×24 CSS px (2.5.8, AA);
the focused element must not be hidden by sticky headers/footers (2.4.11, AA)
([W3C: new in 2.2][w3c-22], [W3C: focus not obscured][w3c-focus]).
→ Minimum 24 px targets (44 px for primary actions on touch), visible
2 px focus ring, `scroll-padding` equal to the sticky header and footer.

**Dark mode.** Follow `prefers-color-scheme`, declare `color-scheme`, use dark
greys rather than pure black, desaturate accents, avoid thin text
([web.dev: color-scheme][webdev-colorscheme], [Material][m3-dark],
[NN/G: dark mode][nng-dark]).
→ Token-based light and dark palettes; the printed sheet is always light.

**Motion.** Respect `prefers-reduced-motion` by reducing (not necessarily
removing) animation ([MDN: media queries for accessibility][mdn-a11y-mq]).
→ Only short opacity/transform transitions, disabled under reduced motion.

**Typography.** Body 15–25 px on screens; 45–90 characters per line, ≤ 80 for
accessibility ([Practical Typography][pt-length], [Baymard: line
length][baymard-length], [W3C WAI: styling][wai-styling]).
→ 16 px body text (Inter), serif display face (Alegreya) for headings and
the sheet's titles, prose capped at ~70ch; fonts are self-hosted (OFL).

## 3. The printed character sheet

**Rank information by how often it changes and how often it is used.**
Designers split sheet content into static, stable and volatile tiers and
give volatile values the most room ([CMU essay][cmu-sheet],
[No Dice Left Behind][ndlb]); infrequently used information should not get
equal visual weight ([EN World discussion][enworld-design]).
→ Page 1 holds everything used every turn (modifiers, saves, skills, AC,
HP, attacks); writable boxes for volatile state (current/temp HP, hit dice
spent, death saves, Heroic Inspiration, exhaustion, conditions, slots).
Features, spells and story follow on later pages.

**Make the number you roll the big number.** Players roll the modifier, not
the score ([EN World][enworld-design]). → Modifiers are the largest figures.

**Follow the 2024 sheet's conventions where they help.** The 2024 sheet
groups skills under their ability, separates class features, species traits
and feats, adds attunement slots, and fits in two pages; reviewers praised the
cleaner layout but noted grouped skills are harder for new players and the
spell area has fewer lines ([Wargamer][wargamer-2024], [GamesRadar][gamesradar],
[EN World thread][enworld-2024]).
→ Skills grouped by ability by default with an alphabetical option; separate
feature sections; three attunement slots; a full spell table with a
"prepared" column and slot boxes; compact two-page mode for non-casters.

**Print like print.** Use `@page` for size and margins, `break-inside: avoid`
for compact blocks (not whole sections), avoid forced breaks that leave empty
pages, don't rely on colour, and test real print output
([MDN: paged media][mdn-paged], [Piccalil][piccalil-print],
[Smashing][smashing-print], [MDN: color][mdn-color]).
→ Letter/A4 switch, 0.4 in margins, every feature/spell card unbreakable,
black-on-white with grey rules (prints in greyscale), minimum 7 pt text,
verified via Chromium's PDF output.

## Tokens

Checked contrast ratios (WCAG 2.x, against the surface they sit on) are
recorded in `crates/web/style.css` next to each colour token.

[nng-wizards]: https://www.nngroup.com/articles/wizards/
[nng-complex]: https://www.nngroup.com/articles/complex-application-design/
[nng-control]: https://www.nngroup.com/articles/user-control-and-freedom/
[nng-progress]: https://www.nngroup.com/articles/progress-indicators/
[nng-skeleton]: https://www.nngroup.com/articles/skeleton-screens/
[nng-empty]: https://www.nngroup.com/articles/empty-state-interface-design/
[nng-radio]: https://www.nngroup.com/articles/checkboxes-vs-radio-buttons/
[nng-placeholder]: https://www.nngroup.com/articles/form-design-placeholders/
[nng-dark]: https://www.nngroup.com/articles/dark-mode-users-issues/
[visa-wizard]: https://design.visa.com/patterns/wizard
[eleken]: https://www.eleken.co/blog-posts/wizard-ui-pattern-explained
[govuk-error]: https://design-system.service.gov.uk/components/error-summary/
[w3c-22]: https://www.w3.org/WAI/standards-guidelines/wcag/new-in-22/
[w3c-focus]: https://www.w3.org/WAI/WCAG22/Understanding/focus-not-obscured-minimum.html
[baymard-inline]: https://baymard.com/blog/inline-form-validation
[baymard-adaptive]: https://baymard.com/research-articles/adaptive-validation-error-messages
[baymard-dropdown]: https://baymard.com/blog/drop-down-usability
[baymard-form]: https://baymard.com/blog/form-design
[baymard-length]: https://baymard.com/blog/line-length-readability
[webdev-contrast]: https://web.dev/learn/accessibility/color-contrast
[webdev-colorscheme]: https://web.dev/articles/color-scheme
[m3-dark]: https://m3.material.io/blog/android-dark-theme-tutorial
[mdn-a11y-mq]: https://developer.mozilla.org/en-US/docs/Web/CSS/Guides/Media_queries/Using_for_accessibility
[mdn-paged]: https://developer.mozilla.org/en/CSS/Paged_Media
[mdn-color]: https://developer.mozilla.org/en-US/docs/Web/CSS/Guides/Colors/Using_color_wisely
[pt-length]: https://practicaltypography.com/line-length.html
[wai-styling]: https://www.w3.org/WAI/tutorials/page-structure/styling/
[cmu-sheet]: https://press4.etc.cmu.edu/file/download/1013/a9e91dcb-16bb-47e0-b844-8dd5dc8ee346
[ndlb]: https://nodiceleftbehind.substack.com/p/rpg-design-notes-3-character-sheet
[enworld-design]: https://www.enworld.org/threads/why-are-character-sheets-so-often-badly-designed.673496/
[enworld-2024]: https://www.enworld.org/goto/post?id=9381772
[wargamer-2024]: https://www.wargamer.com/dnd/redesigned-character-sheet-2024
[gamesradar]: https://gamesradar.com/tabletop-gaming/new-dandd-character-sheet-makes-understanding-skills-easier-and-also-somehow-much-harder
[piccalil-print]: https://piccalil.li/blog/printing-the-web-making-webpages-look-good-on-paper/
[smashing-print]: https://www.smashingmagazine.com/2013/03/tips-and-tricks-for-print-style-sheets/
