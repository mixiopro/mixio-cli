# Recoverable task record

Use this shape for a compact checkpoint, scaling it to the task. Plane remains the work register. Keep private state in the configured shared Link root or the tracking protocol's private retry location; put only appropriate evidence and references on the ticket.

```text
Recorded at / task phase:
Plane ticket + project UUID + issue UUID (verified, or explicitly unavailable):
Accountable owner / execution coordinator / agent role:
Paseo workspace + agent ID:
Repository + branch/worktree + base/current revision:
Outcome / acceptance criteria / important exclusions:
Relevant decisions and source references (scope, date, draft/approved):
Implemented evidence:
Verification performed / failures / skipped checks:
Merge evidence / deployment environment and revision (or unknown):
Uncommitted artifacts and durable recovery location:
Pending external operation / journal / readback result:
Blocker or undecided policy / decision owner:
Next action and expected evidence:
Remaining review, time, cost, or permission boundary:
Plane / Link synchronization result and retry action:
```

Keep the minimum identifiers and evidence needed to resume safely. Omit irrelevant fields; write "unknown" for a material fact that has not been established. Do not infer a person's ownership from seniority or an agent's title.

A recovering agent reads the live ticket and checks current Git/Paseo state. It reconciles pending operations before retrying writes or paid work. Record acknowledgement through the owning task; a cleared or expired Link handoff is not a ticket closure or proof that another agent accepted ownership.
