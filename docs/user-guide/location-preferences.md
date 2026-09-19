# Apply location preferences

This guide shows how to rank the scan locations you prefer to keep, preview the Keep and Remove
decisions that ranking would make across many duplicate-file sets, apply them, and reverse an
application; it assumes you can already record decisions by hand (see
[Review duplicate files and folders](review-duplicates.md)).

A location preference is a saved, reusable rule: "when a file exists in several of these locations,
keep the copy in the highest-ranked one." Rules work on duplicate-file sets from a completed scan.
They record review decisions only; they never delete or check files.

## Save a ranked list of locations

1. Open a completed scan and select the **Review** tab.
2. Expand **Location preferences**.
3. Under **1. Choose preferred locations**, choose an existing rule in **Saved rule**, or select
   **New** to start one. A new rule is named **Preferred scan roots** and lists the scan's locations
   in their recorded order.
4. Enter a **Rule name** of 1 to 128 characters, without spaces at the start or end.
5. Arrange **Roots, highest rank first**:
   - Select a location and use **Move up** or **Move down** to change its rank.
   - Select a location and use **Remove** to take it out of the rule.
   - Type an absolute path in the box below the list and select **Add root** to add it at the
     lowest rank.
6. Select **Save preference**.

A rule holds between 1 and 64 locations. Saving stores the rule only; it does not change any
decision in the current review plan.

## Preview and apply

1. Under **2. Preview the effect**, choose a **Preview scope**:
   - **Current complete filter** — every set that matches the filters applied on **Results** ›
     **Files**.
   - **Selected set** — only the set selected on **Results** › **Files**.
   - **Completed run** — every duplicate-file set in the scan.
2. Select **Preview**. Super Duper lists the affected sets, 100 at a time; select **Next page** to
   see more.
3. Read each row:
   - **Applicable** rows show how many copies the rule would keep and remove, and why. Copies on the
     highest-ranked location are kept; copies on lower-ranked or unranked locations would be marked
     for removal. A copy you already marked **Keep** by hand stays protected.
   - **Blocked** rows explain why the rule cannot act on that set, for example because the result
     would leave no copy to keep, or because a folder you chose to keep contains one of the copies.
4. Select **Apply preview…**.
5. Read the **Confirm saved rule decisions** summary. It gives the number of affected sets and
   copies, the rule Keeps and rule Removes, and the number of blocked sets.
6. Select **Confirm application**, or **Cancel** (or press Esc) to leave decisions unchanged.

The preview itself changes nothing. If you edit or save the rule, change the scope or filter, or
change decisions by hand after previewing, Super Duper asks you to run **Preview** again before
you apply.

After applying, check the result on **Results** › **Files** and on the **Review** tab. See
[Check marked copies before acting on them](check-marked-copies.md) before you act on the plan.

## Reverse an application

1. Under **3. Review applied decisions**, read the latest application for this rule and scan.
2. Select **Reverse rule application**.
3. Select **Confirm reversal**, or **Cancel** (or press Esc) to keep the application.

Reversal clears only the Keep and Remove decisions that this application made. Decisions you made
by hand, including any you made after the application, stay as they are.

Related: [Windows app reference](app-reference.md#review) ·
[Review duplicate files and folders](review-duplicates.md) ·
[How Super Duper finds and reviews duplicates](how-it-works.md#decisions-are-a-plan-not-an-action)
