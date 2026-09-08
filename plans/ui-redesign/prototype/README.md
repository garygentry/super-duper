# Interactive design concept

Open [index.html](index.html) in a browser. It has no dependencies, network requests, database,
filesystem access or persistence. Fictional data resets when reloaded. It is a WPF design reference,
not production frontend code and not a claim of existing product capability.

Use the clearly separate design-review bar to choose Completed, Running, Cancelled, No matches,
Changed copy or First use. Switch the four primary areas, Files/Folders, select sets, expand filters,
mark/reset a copy, open Review, check the fictional plan and inspect warnings. Reset scenario data
with the Reset demo button. Keyboard-native buttons and fields are used throughout.

Demonstrated: information hierarchy, run context, comparison space, reversible decision intent,
filtered result versus entire-plan totals, state-specific messages, review/validation distinction,
and a narrow-layout treatment. Mock paths and dates are intentionally recognizable fiction.

Not implemented in the concept: actual scanning or directory picking; live bytes/ETA; native WPF
controls; sorting/paging beyond the small dataset; saved setup; rule persistence/application;
Explorer integration; worker restart; provider detection; operation recovery; full accessibility
certification. Panels describing these actions are visibly explanatory. No fake executable Recycle
Bin action exists. The full behavior is specified in the parent Markdown documents.

Browser checks are recorded in [verification](../discovery/prototype-verification.md). Native control
metrics, physical DPI and screen-reader behavior must be validated separately on the WPF build.
