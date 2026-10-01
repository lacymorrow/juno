# Permissions by consequence

Juno should ask about what an action does to the world, not about what the
command looks like.

That is the policy. Everything below is the argument for it, an audit of what
Juno actually does today, and the order the change has to happen in.

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

## Nothing is forbidden

There is no tier of acts Juno refuses to do for its owner. `rm -rf`, `sudo`,
disk formatting and the rest are acts that **require permission in every mode,
including the permissive one**. Never silent, never automatic, always possible.
Juno refusing outright to do something its owner asked for is not a decision
this product gets to make for her.

That is a change in kind, not in degree, and it has a trap in it. See "What
must not happen".

## Modes set the threshold

Five modes, spanning further than #645's three, because Lacy asked for both
ends and the ends are real preferences.

| Mode | Asks about |
|---|---|
| Safe | Anything that changes anything, outbound or local, at any size |
| Careful | Outbound, irreversible, or at or above the bulk threshold |
| **Default** | Outbound and irreversible. Reversible local acts go through |
| Trusting | Irreversible only. Outbound goes through below the bulk threshold |
| Permissive | Only the acts in "nothing is forbidden" |

The default is a **rule**, not a point on a volume dial: ask about what leaves
the machine and what cannot be undone. That is why #645's middle mode needs
renaming rather than retuning. Its name, "ask about risky things", describes a
threshold; the thing it should describe is a consequence.

#645's three map onto Safe, Default and Permissive. Careful and Trusting are
new, and they exist because the bulk threshold gives them something to mean
that is not just a louder or quieter version of the default.

## Always allow is load-bearing

It is what makes "ask once before a bulk rename" tolerable rather than
maddening, and it is Lacy's own answer to being asked about system settings
repeatedly. It is no longer a convenience, so its scope matters.

**A grant must be keyed to the consequence class and the target scope, never
to the tool name.** #645 keys it to `(conversation, tool_name)`. With `bash` as
the tool name that is a blanket yes: say "don't ask again" once on an install,
and every other High-risk bash command in that conversation goes through
unasked. Critical still asks, so today the blast is bounded. Make grants
permanent on that key and it stops being bounded.

The scope that is actually wanted:

- **Act**, by consequence class: "moving files in bulk", not "the bash tool".
- **Target**, where there is one: this folder, this recipient, this app.
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

So the enabling work for this whole policy is **give Juno real file tools**:
move, rename, delete, and write, each taking an explicit list of paths. Then
blast radius is `paths.len()`, reversibility is a property of the tool, and the
severity function has something to compute from. Until those exist, every file
act is on the degraded raw-string path, which is the path we distrust.

That reorders the roadmap. Structured file tools come before the severity
function, not after it.

## What changes, in order

### Step 1, shipped here: one place that understands a shell command

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

### Step 2, shipped here: deleting goes to the Trash

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

### Step 3, shipped here: drafts are free

`cli_approval.rs` listed `draft` among the verbs that prompt, so Juno asked
before writing a draft it had just been told to write. Drafting is local and
reversible; sending is not. A connector call that drafts now runs without
asking, unless the same call also sends, forwards, shares, publishes, submits,
invites or deletes. Fleet rules 12 and 13 already make draft-only the
sanctioned path for mail, so this aligns the code with the rule.

### Step 4, next: real file tools

Move, rename, delete and write, each taking an explicit list of paths, each
routed through the Trash where it deletes. This is the prerequisite for
everything below it. Without it there is no count to gate on.

### Step 5, next: the severity function

One function, structured input, three outputs that mean something:
reversibility, reach, blast radius. The shell becomes one input shape among
several rather than the only one, and the substring tests shrink to the
degraded fallback they should always have been.

### Step 6, next: `sudo` becomes askable, carefully

Under "nothing is forbidden", Gate 2's `sudo` refusal becomes an approval. The
trap: Gate 2 is not an approval gate, it is a crash barrier on a code path with
no human attached. Making `sudo` askable means **adding the approval route
first** and removing the refusal second. Done in the other order, "nothing is
forbidden" quietly becomes "nothing is checked". The refusal is the thing
holding the line until the question exists.

### Step 7, next: the sentence never names a command

`describe_action` still prints `Run this in the terminal: <command>`. Under this
policy a shell command that reaches a prompt is an install, a network-piped
script, or an escalation, and each can be said in her terms: "Install
left-pad?", "Run a script downloaded from example.com?", "Do this as an
administrator?". No path, no tool name, no risk level, no shell syntax.

### Step 8, next: the five modes and the grant surface

The mode span above, the grant keyed to consequence class and target, and the
place to revoke a permanent grant. The revoke surface ships with the grant.

## What must not happen

Moving a category from "ask" to "safe" is only legitimate once the construction
exists and is tested. Removing the delete question without routing deletes to
the Trash would be strictly worse than today. Removing Gate 2's refusals
because "nothing is forbidden" before the approval route exists would be worse
still, and it is the single most likely way to get this wrong.

#645's 60 bypass assertions stay green through every step. If one ever becomes
the wrong test, the change that alters it says why in the test itself.
