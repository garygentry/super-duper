# Dedicated VM background UI iteration (2026-09-15)

The operator explicitly requested that development continue when the remote Windows desktop is
backgrounded or possibly locked. Computer Use native input reported `GetCursorPos` access denied,
and `LogonUI` was active. This record tests the independent WPF path; it does not claim native
mouse/keyboard control of the live app in that state or reopen a completed redesign gate.

In the VM's normal development context, the Debug loaded-STA WPF smoke project passed all three
methods with `SUPER_DUPER_UIR05C_CAPTURES` pointing to ignored
`artifacts/vm-background-captures`. It produced 125 PNGs. The narrow 900 × 600 File comparison and
Review Location preferences application captures were opened and inspected; the fixture rendered
its controls and the target Review action was reached by programmatic scrolling.

The complete Debug Windows solution suite also passed in the same inaccessible-desktop state:
220 Core, 76 Infrastructure and three WPF methods, with five expected physical/provider/deletion
skips. The read-only Recycle Bin root eligibility check requires the VM's normal development
context; the filesystem sandbox alone blocks its query. No real Recycle Bin execution, physical
drive campaign, provider transfer or production-state operation ran. No app, worker or fixture
process was left open.

Repository guidance now lets agents use builds, tests, isolated workers, loaded-STA WPF control and
rendered captures for authorized UI iteration while native input is unavailable. Any gate that
explicitly requires physical desktop evidence still waits for that separate evidence.
