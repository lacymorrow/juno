# Permissions by consequence

Juno allows. It asks before it sends a communication and before it spends
money, and otherwise it gets on with the work.

That is the policy. Everything below is the argument for it, an audit of what
Juno actually does today, and the order the change has to happen in.

## The directive, which reverses the posture

Lacy, 2026-10-01:

> "Adoption must be widespread, I prefer to start by allowing Juno to do
> everything and then we can rein in permissions as users privacy concerns
> arise."

So the default **allows**. Not "a cautious default with a permissive option
available": the permissive posture *is* the default. A computer-use app that
interrupts is one nobody keeps, and the target user will not know how to answer
a prompt, so every default that asks is a default that stops her.

Two exceptions, set the same day and standing:

- **Sending a communication.** "It should not send communications out without
  approval but it should draft emails freely."
- **Spending money.**

Both for one reason: they leave the machine and cannot be recalled. Everything
local goes through without a question.

An earlier draft of this document implied the opposite posture, with a graded
mode table and a list of acts that always ask. That is reversed. The graded
modes still have a place, but they are what we **rein in with, later, in
response to an actual privacy report**, not what we ship first. Do not build an
elaborate mode system before the default is right.

## Safety comes from construction, not from questions

This is the organising principle, not one tactic among several.

`rm` to the Trash is the model. Deleting stopped being a question not because
the question got better copy but because the act became reversible. The shim
made it undoable, and then the prompt had nothing to protect.

So, before removing any prompt: **name the construction that makes the act
safe, and build that.** If no construction exists, the prompt stays. Never
remove a prompt and leave the hazard. The failure mode this rule exists to
prevent is the one that would actually hurt someone: a permissive default
layered on top of a removed safeguard, which is not "permissive", it is
unchecked.

The same rule in the other direction: `sudo`, disk formatting and `rm -rf /`
have **no** construction that makes them reversible. Nothing undoes them. So
they are not covered by the permissive default by default, and the ordering in
"What changes, in order" is strict about how they become askable.

## Who the default is for

Someone who installed Juno to talk to her Mac. She does not know what bash is,
she will never open Settings, and when a dialog appears mid-sentence she will
either press the button that makes it go away or stop using Juno. She is who
the default has to be right for. A middle setting that a careful person would
adjust is not a default, it is a deferral.

## Severity is computed, not looked up

Three inputs, and none of them is "what does the command look like".

1. **Reversibility in practice.** Can she undo it herself, easily, without
   Juno. Deleting to the Trash is reversible. Emptying the Trash is not. Four
   hundred renames is technically reversible and practically not.
2. **Reach.** Does it stay on this machine, or does it leave and touch other
   people. Drafting stays. Sending leaves and cannot be recalled.
3. **Blast radius.** How much is affected at once. One file or a directory
   tree. The act is identical; the consequence is not.

Reversibility is not binary, and blast radius is why. This is also the
strongest argument against pattern-matching command strings: blast radius is a
count. A tool call knows how many paths it was given, where it is writing, and
whether there is a recipient. That is structured data you can gate on exactly.
You cannot get a count out of a shell command with a regex, and the attempt is
what makes the current classifier brittle.

So: compute severity from the structured parameters of the call. Inspect a raw
command string only where there is no structure to read, and treat that as the
degraded path it is.

### The bulk threshold

`BULK_CHANGE_THRESHOLD = 10`.

Five file moves is noise to ask about; two hundred must ask. Ten is where
undoing by hand stops being a few seconds and becomes a chore, and it is about
as many items as a person can still check in a single glance at a list. It is a
named constant so the next person can argue with the number instead of
excavating it from a branch.

At or above the threshold, an act that would not otherwise ask asks **once,
before the batch**, names the count in her terms, and offers always-allow. It
does not ask again in that task after a yes. Nothing has moved when she
declines.

Under a permissive default this is the **only** prompt outside the two
exceptions, which makes it the one place the permissive posture is still
willing to interrupt, and it is the reason structured file tools are the
critical path rather than a tidy-up. A sweeping irreversible change is exactly
what Juno cannot currently see, because nothing it registers takes a list of
paths. Until that exists this threshold is a number with nothing to measure.

## What the default permits, and the two things it does not

**Permitted, silently, in the default.** Delete (to the Trash), move, rename,
create, write and edit files. Draft an email, message, post or comment. Open,
close and focus apps. Read anything local. Browse, search and read pages. Fill
a form. Run a shell command, inert or not. Install software. Change a system
setting. Take a screenshot, move the mouse, type. None of this asks.

That list is deliberately not hedged. Installing software and changing a system
setting are both on it, and both sat in the "always ask" column of an earlier
draft. They are local, a person can undo them, and asking about `npm install`
is exactly the prompt flood #645 was fixing. Lacy would not expect to be asked
every time something touches a setting, and "always allow" is his own answer to
the cases where he would.

**Gated, in every mode including the permissive one:**

1. **Sending a communication.** Email, message, post, comment, reply. The
   question names the recipient and the first line, so it reads "Send this to
   Carol?" rather than "Allow a connector write?".
2. **Spending money.** Buy, book, pay, subscribe, transfer, or submit a form
   that transacts.

**Sharing and granting access count as sending**, not as a third exception.
Making a file public or inviting someone to it puts data in front of a person
who could not see it before, which is the same consequence under a different
verb. Treating that as part of the definition keeps the exception set at two.

### One act I argue to add, and one I argue to leave out

**Add: emptying the Trash.** It is the single act that defeats the construction
this whole policy rests on. Every delete Juno made reversible becomes
irreversible, retroactively and in bulk, and nobody asking Juno to tidy a
folder is asking for that. It costs nothing: nobody asks Juno to empty the
Trash in the course of ordinary work, so the prompt will almost never fire. A
gate that defends a construction and fires approximately never is the cheapest
safety in this document.

**Leave out: installing software, and running a script fetched from the
network** (`curl | sh`, `brew install`, `npm install`). The argument for gating
it is real: it executes third-party code with her privileges, she cannot undo
it, and it is the main route by which a prompt-injected model does lasting
damage. I am not arguing for it, because it is ordinary work on a developer's
machine, the prompt would fire constantly, and a prompt that fires constantly
trains people to dismiss prompts. That costs more safety than it buys.
Recorded here so it is a decision rather than an oversight. If it is ever
revisited, the thing to build first is an install log Juno can roll back, not a
dialog.

## Nothing is forbidden, and the ordering that makes that safe

There is no tier of acts Juno refuses to do for its owner. `rm -rf`, `sudo` and
disk formatting become acts that **require permission in every mode, including
the permissive one**. Never silent, never automatic, always possible. Juno
refusing outright to do something its owner asked for is not a decision this
product gets to make for her.

The trap is the interaction with a permissive default, and it is the one
outcome that would actually cause harm: **a permissive default on top of a
removed crash barrier is not permissive, it is unchecked.** So the ordering is
strict and not negotiable.

1. Build the approval route: a consequence class for escalation and
   irreversible disk acts, a sentence a person can answer, and a floor no mode
   waives.
2. Only then remove the refusal in `commands::shell::refuse_forbidden_command`.

Until step 1 exists, that gate keeps refusing, and the permissive default
changes nothing about it. These acts have no construction that makes them
reversible, which is exactly why the prompt cannot be removed and left empty.

## Modes, deferred on purpose

Five modes were agreed and recorded: Safe, Careful, Default, Trusting,
Permissive. They are not the next slice.

What matters now is that the default is right. A permissive mode must exist and
the default must be it or very near it. A Safe mode is **what we rein in with,
later, in response to an actual privacy report**, not what we ship first.
Building a graded mode system before the default is correct is building the
answer to a question nobody has asked yet.

#645's three modes survive as the ends plus the middle. The middle one needs
renaming rather than retuning: "ask about risky things" describes a threshold,
and the thing it should describe is a consequence.

## Always allow, now narrow but still load-bearing

A permissive default takes most of the pressure off this. If Juno stops asking
about shell work, a blanket `(conversation, "bash")` grant has almost nothing
left to leak, so #645's scope is mostly moot.

It is not entirely moot, and where it still matters it matters more:
**the two exceptions are exactly where a grant keyed to a tool name instead of
a consequence and a target would be a real hole.** "Always allow" on emailing
Carol must not authorise emailing anyone else, and "always allow" on a $4
subscription must not authorise a $4,000 one.

So the grant key for the gated set is `(conversation, consequence class,
target)`:

- **Consequence class**, not tool name: "sending a message", not `bash` and not
  `mcp__gmail__send_email`.
- **Target**: the recipient for a send, the payee or domain for a spend.
- **Lifetime**: the task by default, permanent only on an explicit choice.

A grant that can outlive the session needs a visible place to revoke it, and
that surface ships in the same change as the grant. Not promised, shipped.

## What Juno asks about today

Audited at v0.8.44. There are **three** permission mechanisms, not two, and
they do not agree.

### Gate 1, in-process: does Juno ask

`agent/tools/risk_classifier.rs` assigns a risk level and
`agent/tools/permission_policy.rs::requires_approval` turns that plus the mode
into a yes or no, from `agent_runner.rs`. Shell risk comes from 23 substring
tests over the command text.

### Gate 2, in-process: does Juno refuse outright

`commands/shell.rs`, renamed in this change to `refuse_forbidden_command`.

This is a **blocklist, not an allowlist**, despite `src-tauri/CLAUDE.md` calling
it "command whitelist validation" in two places and illustrating it with an
`ALLOWED_COMMANDS` constant that exists nowhere in Juno. That documentation is
where the "two allowlists" report came from.

It refuses eight catastrophic literals, recursive forced `rm` of `/` in any
flag permutation, `sudo` and `doas` anywhere in the text, redirection into
`/etc`, `/bin`, `/usr/bin`, `/usr/local/bin` and `/System`, path traversal in a
redirection, and anything over 10,000 characters.

### Gate 3, Claude CLI path: does Juno ask before it sends

`agent/providers/cli_approval.rs` (LAC-4058). **This gate already implements
the policy in this document.** It routes the CLI's prompts into Juno's own
sheet, pre-approves the everyday local tools, auto-allows read-only connector
calls, and prompts for anything that sends, schedules, deletes or writes on the
other side. It gates by consequence where intent is structurally known, and it
fails closed on anything it does not recognise.

Juno has one gate built the right way and two built the wrong way. The
consolidation is not "merge the lists". It is "make the in-process path work
the way the CLI path already does".

### Spending has no gate at all, today

Worth stating plainly, because the permissive default does not create this hole
and must not be blamed for it: **Juno cannot currently tell that it is about to
spend money.**

`browser_interact` takes `click`, `type`, `select` and `scroll`. There is no
`submit`. Clicking a "Pay" button is `click`, which classifies Low and runs
silently now and under any mode. The only thing that incidentally catches a
purchase is `classify_form_fill_risk`, which goes Critical when a card number
or CVV passes through a `type` action. If the card is already saved in the
browser, nothing fires.

`classify_browser_nav_risk` flags checkout, payment and bank URLs High, but
that is navigation, not spending, and navigating to a checkout page is
something the permissive default should let through.

So the spending exception is not a prompt to preserve. It is a gate to build,
and it has to be built on something structural rather than on a URL substring.
That is step 5.

### The second allowlist, and it is dead

`cloud/config.rs` holds `static ALLOWED_COMMANDS` and a
`CloudConfig::allowed_commands` field. It is never read: `is_command_allowed`
consults only the denied list and then returns `true`. It does not hold shell
commands either; it holds cloud message type names (`text_query`, `heartbeat`,
`screenshot`). A dead control named like a live one is the recurring defect in
this codebase. Removed in this change.

## The prerequisite nobody has named: Juno has no file tools

Severity computed from structured parameters needs structure to read. Juno
does not have it.

The tools Juno actually registers for files are `read_file`,
`smart_create_file`, `str_replace_based_edit_tool` and `save_and_close_file`.
There is **no move, no rename, no delete, and nothing that takes a list of
paths.** The downloads-folder example in the brief, 200 files into dated
subfolders, happens today as a `for` loop in bash. There is no count to read,
no target list, no structure at all.

Worse, the classifier's file arms (`write_file`, `edit_file`, `create_file`,
`str_replace_editor`, `delete_file`, `remove_file`, `unlink_file`) are for
tools Juno **does not register**. This is the same dead-name defect #645
removed from the form-fill arm, and it means today's "a file write is Medium so
Safe mode sees it" rule protects nothing: a real file write arrives as
`smart_create_file`, `str_replace_based_edit_tool`, or bash, and none of the
three is classified.

`write_file` and `open_url` do appear as `ToolDefinition`s, in
`src/tools/mod.rs::list_tools`. That function has **no callers**: the other
`list_tools` hits in the tree are trait methods on the real tool providers. It
is a catalogue of around twenty tool definitions that nothing registers, which
is why grepping for the name finds something and the model never sees it. It is
also why the classifier's file arms looked plausible for so long.

So the enabling work for this whole policy is **give Juno real file tools**:
move, rename, delete, and write, each taking an explicit list of paths. Then
blast radius is `paths.len()`, reversibility is a property of the tool, and the
severity function has something to compute from. Until those exist, every file
act is on the degraded raw-string path, which is the path we distrust.

That reorders the roadmap. Structured file tools come before the severity
function, not after it.

## What changes, in order

### Step 1, shipped in #648: one place that understands a shell command

`src-tauri/src/shell_command.rs`. Normalisation, tokenisation, the inert
character whitelist, the command word, and the recursive-forced-`rm` tokeniser
live here and nowhere else. Gate 1, Gate 2 and both cloud checks call it.

Before this, Gate 2 normalised the command and Gate 1 did not, which is a live
disagreement: `rm  -rf  /` with two spaces was refused outright by Gate 2 and
classified merely High by Gate 1. Two parsers of the same syntax drift, and the
drift is silent.

Nothing is newly permitted. Where the shared layer changes an answer it only
ever refuses more: Gate 1 now sees the normalised form as well as the raw text,
and gained Gate 2's `rm` flag-permutation tokeniser.

This step does not make the classifier better. It makes it **one thing**, so
the step that shrinks it only has to be done once.

### Step 2, shipped in #648: deleting goes to the Trash

Juno has no trash path today and no delete tool, so the only way Juno deletes
anything is `rm` in the bash session. The construction goes where the deleting
happens: the session now runs its own `rm`.

`ShellSession` writes a `bin/rm` shim into its session directory and prepends
that directory to `PATH` in the session's init commands. `rm` resolves to the
shim, which hands each target to `/usr/bin/trash`.

This is deliberately **not** string surgery. Juno does not rewrite `rm` into
`trash`; it changes what `rm` is. Bash still does all the parsing, so globs,
quoting, chaining, variables and `xargs` are expanded by bash before the shim
sees a path. There is nothing left to guess, which is the whole point.

Failure modes, designed rather than inherited:

- **The Trash cannot take it** (network volume, read-only mount, no Trash for
  that path): the shim says deleting would be permanent and **exits non-zero
  without deleting**. It never falls back to deletion. The model sees the
  refusal and can ask.
- **`/usr/bin/trash` missing** (older macOS): same answer. Refuse, do not
  delete.
- **`-f` on something absent**: exits 0 silently, the way `rm -f` does, so
  build scripts keep working.
- **A target that exists and fails to trash**: non-zero, named, not deleted.
- **`/bin/rm` by absolute path** bypasses the shim. Gate 2 still refuses the
  catastrophic forms, which is why Gate 2 stays. Defence in depth.
- **Child processes** inherit `PATH`, so `rm` inside a `make` or `npm` script
  also goes to the Trash. That is the intent. The cost is Trash volume, which
  is recoverable; permanent deletion is not.

### Step 3, shipped in #648: drafts are free

`cli_approval.rs` listed `draft` among the verbs that prompt, so Juno asked
before writing a draft it had just been told to write. Drafting is local and
reversible; sending is not. A connector call that drafts now runs without
asking, unless the same call also sends, forwards, shares, publishes, submits,
invites or deletes. Fleet rules 12 and 13 already make draft-only the
sanctioned path for mail, so this aligns the code with the rule.

### Step 4, next slice: the permissive default

The default stops asking about local work. Two pieces, and the second is the
one that carries the risk.

**4a. `cli_approval` allows by default.** This is where most of the asking
happens today, and it is the cheapest change with the biggest effect, because
it already gates by consequence at the tool boundary. The verb lists invert:
instead of "a write verb prompts", the rule becomes "a send or a spend prompts,
and everything else runs". That retires the prompts on `mark`, `archive`,
`move`, `pin`, `star`, `react`, `assign`, `close`, `update`, `create`, `add`
and `set`, each of which is local or reversible on the other side.

The verb matching stays, and stays failing closed on the *gated* set: a name
Juno cannot read as either a send or a spend runs, but a name that reads as
either one prompts. That is the correct direction now, because the default is
permissive: the thing to be conservative about is the two exceptions, not
everything else.

**4b. `requires_approval` becomes a consequence rule.** Today it compares a
risk level against a mode. It needs a consequence class instead, because
"ask on High" no longer means anything: High is `rm <file>` and `npm install`,
both of which now go through.

The trap, and it is the whole reason this is one slice and not a one-line
change: **deleting the High branch without adding the send and spend classes
first would permit a payment click and a connector send in the same move.**
Spending has no gate today (see the audit above), so there is nothing to
preserve and everything to build. The order inside this step is: add the
classes, prove them with tests, then change the default.

What stays asking after 4b, and why:

| Act | Why it still asks |
|---|---|
| Send a communication | Leaves the machine, cannot be recalled |
| Spend money | Same, plus it costs her money |
| Empty the Trash | Defeats the construction every other delete relies on |

What `refuse_forbidden_command` still refuses after 4b: unchanged. `sudo`,
`doas`, the catastrophic literals, recursive forced `rm` of `/`, redirection
into system directories. Those become askable in step 7, not here.

### Step 5, next: structured file tools, the critical path

This is the prerequisite, as established above, and under a permissive default
it matters more rather than less: **the one place we would still want to ask, a
sweeping irreversible change, is exactly the place Juno currently cannot
detect.** There is no count to read because there is no tool that takes a list
of paths.

Scope, in order, each its own PR:

1. **Hoist the Trash primitive into Rust.** `/usr/bin/trash` is currently
   invoked only from the generated shim script. Lift it to a function the shim
   and the tools both use, so there is one definition of "delete safely" and
   one place where "the Trash cannot take this" is decided.
2. **Four tools**, each taking an explicit `paths: [String]`: `move_paths`,
   `rename_path`, `delete_paths` (through the primitive), `write_file`. Each
   routed through the existing `agent/tools/path_security.rs` for
   canonicalisation and workspace bounds. `read_file` already exists.
3. **Replace the dead classifier arms.** `write_file`, `edit_file`,
   `create_file`, `str_replace_editor`, `delete_file`, `remove_file` and
   `unlink_file` are arms for tools Juno does not register; they go, and arms
   for the real names replace them. Pinned with a test that reads the tool
   registry, the way `the_registered_browser_typing_tool_is_classified` does,
   so the arms cannot drift from the registry again.
4. **Blast radius.** `paths.len()` against `BULK_CHANGE_THRESHOLD`. Ask once,
   before the batch, naming the count. This is the first prompt this document
   *adds*, and it is only legitimate because step 2 gives it a real number
   rather than a guess from a command string.

Note the ordering inside this step: the tools come before the counting, and the
classifier cleanup comes before the counting too, because counting on arms that
point at nothing would be a safeguard disconnected from what it names.

### Step 6, next: the sentence never names a command

`describe_action` still prints `Run this in the terminal: <command>`. Under a
permissive default a shell command barely ever reaches a prompt, so the
remaining sentences are the two exceptions, and both can be said in her terms:
"Send this to Carol?", "Pay $24 to example.com?". No path, no tool name, no
risk level, no shell syntax. If a sentence has to name a command, the gate is
in the wrong place.

### Step 7, next: `sudo` becomes askable

The approval route first, the refusal second. See "Nothing is forbidden, and
the ordering that makes that safe". This is deliberately last among the gate
changes, because it is the step where getting the order wrong converts a
permissive default into an unchecked one.

### Step 8, later: modes and the grant surface

The five-mode span, the grant keyed to consequence and target, and the place to
revoke a permanent grant. Deferred until the default is right and someone has
actually asked to be asked more.

## What must not happen

A permissive default is not a licence to remove safeguards. It is a statement
about **questions**, not about protections, and the two are not the same thing.

Three specific ways to get this wrong, in order of how likely they are:

1. **Removing Gate 2's refusals because "nothing is forbidden", before the
   approval route exists.** That is not permissive, it is unchecked, and it is
   the one outcome that would cause real harm. The route first, the refusal
   second, always.
2. **Deleting the "ask on High" branch without first building the send and
   spend classes.** High is where connector sends and payment clicks would fall
   through, and spending has no gate of its own today, so this is a one-line
   change that silently permits buying things.
3. **Removing a prompt for an act with no construction behind it.** Deleting
   stopped asking because `rm` goes to the Trash. Nothing else gets to stop
   asking on the strength of an intention to build the construction later.

#645's 60 bypass assertions stay green through every step. If one ever becomes
the wrong test, the change that alters it says why in the test itself.
